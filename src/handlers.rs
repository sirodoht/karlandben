use crate::{
    database,
    email::EmailService,
    models::{EmailForm, NameForm, VerifyForm},
    services::generate_code,
    session::{self, Session},
    templates::{IndexTemplate, LoginTemplate, ProfileTemplate, VerifyTemplate},
};
use askama::Template;
use axum::{
    Extension,
    extract::Form,
    http::header,
    response::{Html, IntoResponse, Redirect},
};
use sqlx::SqlitePool;
use tracing::{debug, error, info, instrument, warn};

#[instrument(skip(session), fields(user_id, has_name))]
pub async fn index(session: Session) -> impl IntoResponse {
    tracing::Span::current()
        .record("user_id", session.user_id.unwrap_or(0))
        .record("has_name", session.user_name.is_some());

    // If user is logged in but has no name, redirect to profile completion
    if session.is_authenticated() && session.user_name.is_none() {
        debug!(
            user_id = session.user_id.unwrap_or(0),
            "User has no name, redirecting to /profile"
        );
        return Redirect::to("/profile").into_response();
    }

    let tmpl = IndexTemplate {
        logged_in: session.is_authenticated(),
        name: session.user_name,
        email: session.email,
    };
    Html(tmpl.render().unwrap()).into_response()
}

pub async fn login_page() -> impl IntoResponse {
    Html(
        LoginTemplate {
            error: None,
            logged_in: false,
            email: None,
        }
        .render()
        .unwrap(),
    )
}

#[instrument(skip(pool, email_service, form), fields(email))]
pub async fn login(
    Extension(pool): Extension<SqlitePool>,
    Extension(email_service): Extension<Option<EmailService>>,
    Form(form): Form<EmailForm>,
) -> impl IntoResponse {
    let email = form.email.trim().to_lowercase();
    tracing::Span::current().record("email", email.as_str());

    info!("Login attempt initiated");

    // Validate email format
    if !email.contains('@') {
        return Html(
            LoginTemplate {
                logged_in: false,
                email: None,
                error: Some("Invalid email address".to_string()),
            }
            .render()
            .unwrap(),
        )
        .into_response();
    }

    // Check rate limit
    let allowed = match database::check_rate_limit(&pool, &email).await {
        Ok(a) => {
            if !a {
                warn!(email = %email, "Rate limit exceeded for login");
            }
            a
        }
        Err(_) => {
            error!("Database error checking rate limit");
            return Html(
                LoginTemplate {
                    logged_in: false,
                    email: None,
                    error: Some("Database error".to_string()),
                }
                .render()
                .unwrap(),
            )
            .into_response();
        }
    };

    if !allowed {
        return Html(
            LoginTemplate {
                logged_in: false,
                email: None,
                error: Some(
                    "Too many attempts. Please wait before requesting another code.".to_string(),
                ),
            }
            .render()
            .unwrap(),
        )
        .into_response();
    }

    // Create token and send email
    let code = generate_code();
    match database::create_token(&pool, &email, &code).await {
        Ok(_) => {
            info!(email = %email, "Sign-in token created successfully");
        }
        Err(e) => {
            error!(error = ?e, "Failed to create sign-in token");
            return Html(
                LoginTemplate {
                    logged_in: false,
                    email: None,
                    error: Some("Failed to create sign-in code".to_string()),
                }
                .render()
                .unwrap(),
            )
            .into_response();
        }
    };

    // Send email (don't fail if email service is not configured)
    if let Some(service) = email_service {
        match service.send_sign_in_code(&email, &code).await {
            Ok(_) => info!(email = %email, "Sign-in code email sent successfully"),
            Err(e) => {
                error!(email = %email, error = %e, "Failed to send sign-in code email");
                return Html(
                    LoginTemplate {
                        logged_in: false,
                        email: None,
                        error: Some(
                            "Unable to send verification email. Please try again later."
                                .to_string(),
                        ),
                    }
                    .render()
                    .unwrap(),
                )
                .into_response();
            }
        }
    } else {
        info!(email = %email, code = %code, "[DEV] Sign-in code (email not configured)");
    }

    Html(
        VerifyTemplate {
            logged_in: false,
            form_email: email,
            email: None,
            error: None,
        }
        .render()
        .unwrap(),
    )
    .into_response()
}

#[instrument(skip(pool, form), fields(email))]
pub async fn verify(
    Extension(pool): Extension<SqlitePool>,
    Form(form): Form<VerifyForm>,
) -> impl IntoResponse {
    let email = form.email.trim().to_lowercase();
    let code = form.code.trim();

    tracing::Span::current().record("email", email.as_str());

    info!("Verification attempt");

    // Validate token
    let valid = match database::validate_and_use_token(&pool, &email, code).await {
        Ok(v) => {
            if v {
                info!("Token validated successfully");
            } else {
                warn!("Invalid or expired token submitted");
            }
            v
        }
        Err(e) => {
            error!(error = ?e, "Database error validating token");
            return Html(
                VerifyTemplate {
                    logged_in: false,
                    form_email: email.clone(),
                    email: None,
                    error: Some("Database error".to_string()),
                }
                .render()
                .unwrap(),
            )
            .into_response();
        }
    };

    if !valid {
        return Html(
            VerifyTemplate {
                logged_in: false,
                form_email: email,
                email: None,
                error: Some("Invalid or expired code".to_string()),
            }
            .render()
            .unwrap(),
        )
        .into_response();
    }

    // Get or create user (creates user with NULL name if doesn't exist)
    let (user_id, _was_created) = match database::get_or_create_user(&pool, &email).await {
        Ok(result) => result,
        Err(_) => {
            return Html(
                VerifyTemplate {
                    logged_in: false,
                    form_email: email,
                    email: None,
                    error: Some("Database error".to_string()),
                }
                .render()
                .unwrap(),
            )
            .into_response();
        }
    };

    // Create session immediately - user now has an account
    let token = session::generate_session_token();
    let max_age_days = session::SESSION_MAX_AGE_DAYS;
    if let Err(e) = database::create_session(&pool, user_id, &token, max_age_days).await {
        error!(error = ?e, "Failed to create session");
        return Html(
            VerifyTemplate {
                logged_in: false,
                form_email: email.clone(),
                email: None,
                error: Some("Database error".to_string()),
            }
            .render()
            .unwrap(),
        )
        .into_response();
    }

    info!(user_id, email = %email, "User authenticated - session created");

    // Check if user needs to set name (has no name yet)
    let needs_name = match database::user_needs_name(&pool, &email).await {
        Ok(n) => n,
        Err(_) => {
            // Even if check fails, user has a session - still redirect them somewhere
            // Create response with session cookie, redirect to home
            let mut response = Redirect::to("/").into_response();
            if let Some(cookie) = session::build_session_cookie(&token) {
                response.headers_mut().insert(header::SET_COOKIE, cookie);
            }
            return response;
        }
    };

    // Build response with session cookie
    let mut response = if needs_name {
        // User has account but needs to complete profile
        info!(user_id, email = %email, "User needs to complete profile - redirecting to /profile");
        Redirect::to("/profile").into_response()
    } else {
        // User has complete profile
        info!(user_id, email = %email, "User signed in successfully");
        Redirect::to("/").into_response()
    };

    if let Some(cookie) = session::build_session_cookie(&token) {
        response.headers_mut().insert(header::SET_COOKIE, cookie);
    }
    response
}

#[instrument(skip(session))]
pub async fn profile_page(session: Session) -> impl IntoResponse {
    // Check if user is authenticated
    if !session.is_authenticated() {
        info!("Unauthenticated user attempted to access profile page - redirecting to login");
        return Redirect::to("/login").into_response();
    }

    let email = session.email.clone().unwrap_or_default();

    Html(
        ProfileTemplate {
            form_email: email.clone(),
            email: session.email,
            error: None,
            logged_in: true,
            name: session.user_name,
        }
        .render()
        .unwrap(),
    )
    .into_response()
}

#[instrument(skip(pool, session, form), fields(name_len))]
pub async fn register_name(
    Extension(pool): Extension<SqlitePool>,
    session: Session,
    Form(form): Form<NameForm>,
) -> impl IntoResponse {
    // Check if user is authenticated
    if !session.is_authenticated() {
        info!("Unauthenticated user attempted to register name - redirecting to login");
        return Redirect::to("/login").into_response();
    }

    let email = session.email.clone().unwrap_or_default();
    let name = form.name.trim();

    tracing::Span::current().record("name_len", name.len());

    info!(email = %email, "Name registration initiated");

    if name.is_empty() {
        warn!("Empty name submitted");
        return Html(
            ProfileTemplate {
                form_email: email.clone(),
                email: session.email,
                error: Some("Name is required".to_string()),
                logged_in: true,
                name: session.user_name,
            }
            .render()
            .unwrap(),
        )
        .into_response();
    }

    // Update user with name
    match database::update_user_name(&pool, &email, name).await {
        Ok(_) => {
            info!(email = %email, "User name updated successfully");
            Redirect::to("/").into_response()
        }
        Err(_) => {
            error!(email = %email, "Database error updating user name");
            Html(
                ProfileTemplate {
                    form_email: email.clone(),
                    email: session.email,
                    logged_in: true,
                    error: Some("Database error".to_string()),
                    name: session.user_name,
                }
                .render()
                .unwrap(),
            )
            .into_response()
        }
    }
}

#[instrument(skip(session))]
pub async fn logout(Extension(pool): Extension<SqlitePool>, session: Session) -> impl IntoResponse {
    // Delete session from database if exists
    if let Some(token) = session.token
        && let Err(e) = database::delete_session(&pool, &token).await
    {
        error!(error = ?e, "Failed to delete session");
    }

    if let Some(user_id) = session.user_id {
        info!(user_id, "User logged out");
    }

    // Build response with cleared cookie
    let mut response = Redirect::to("/").into_response();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, session::clear_session_cookie());
    response
}
