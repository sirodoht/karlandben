use crate::{
    database,
    email::EmailService,
    models::{EmailForm, NameForm, VerifyForm},
    services::generate_code,
    session::{self, Session},
    templates::{IndexTemplate, LoginTemplate, RegisterNameTemplate, SignupTemplate, VerifyTemplate},
};
use askama::Template;
use axum::{
    extract::Form,
    http::header,
    response::{Html, IntoResponse, Redirect},
    Extension,
};
use sqlx::SqlitePool;
use tracing::{debug, error, info, instrument, warn};

#[instrument(skip(session), fields(user_id, has_name))]
pub async fn index(session: Session) -> Html<String> {
    tracing::Span::current()
        .record("user_id", &session.user_id.unwrap_or(0))
        .record("has_name", session.user_name.is_some());

    let tmpl = IndexTemplate {
        logged_in: session.user_id.is_some(),
        name: session.user_name,
    };
    Html(tmpl.render().unwrap())
}

pub async fn login_page() -> impl IntoResponse {
    Html(LoginTemplate { error: None }.render().unwrap())
}

#[instrument(skip(pool, email_service, form), fields(email))]
pub async fn login(
    Extension(pool): Extension<SqlitePool>,
    Extension(email_service): Extension<Option<EmailService>>,
    Form(form): Form<EmailForm>,
) -> impl IntoResponse {
    let email = form.email.trim().to_lowercase();
    tracing::Span::current().record("email", &email.as_str());

    info!("Login attempt initiated");

    // Validate email format
    if !email.contains('@') {
        return Html(
            LoginTemplate {
                error: Some("Invalid email address".to_string()),
            }
            .render()
            .unwrap(),
        )
        .into_response();
    }

    // Check if user exists
    let exists = match database::user_exists(&pool, &email).await {
        Ok(e) => {
            if e {
                debug!("User exists in database");
            } else {
                warn!(email = %email, "Login attempt for non-existent user");
            }
            e
        }
        Err(_) => {
            error!("Database error checking user existence");
            return Html(
                LoginTemplate {
                    error: Some("Database error".to_string()),
                }
                .render()
                .unwrap(),
            )
            .into_response()
        }
    };

    if !exists {
        return Html(
            LoginTemplate {
                error: Some("No account found. Please sign up first.".to_string()),
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
                    error: Some("Database error".to_string()),
                }
                .render()
                .unwrap(),
            )
            .into_response()
        }
    };

    if !allowed {
        return Html(
            LoginTemplate {
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
    match database::create_token(&pool, &email, &code, "login").await {
        Ok(_) => {
            info!(email = %email, "Login token created successfully");
        }
        Err(e) => {
            error!(error = ?e, "Failed to create login token");
            return Html(
                LoginTemplate {
                    error: Some("Failed to create login code".to_string()),
                }
                .render()
                .unwrap(),
            )
            .into_response();
        }
    };

    // Send email (don't fail if email service is not configured)
    if let Some(service) = email_service {
        match service.send_login_code(&email, &code).await {
            Ok(_) => info!(email = %email, "Login code email sent successfully"),
            Err(e) => {
                error!(email = %email, error = %e, "Failed to send login code email");
                return Html(
                    LoginTemplate {
                        error: Some("Unable to send verification email. Please try again later.".to_string()),
                    }
                    .render()
                    .unwrap(),
                )
                .into_response();
            }
        }
    } else {
        info!(email = %email, code = %code, "[DEV] Login code (email not configured)");
    }

    Html(
        VerifyTemplate {
            email,
            purpose: "login".to_string(),
            error: None,
        }
        .render()
        .unwrap(),
    )
    .into_response()
}

pub async fn signup_page() -> impl IntoResponse {
    Html(SignupTemplate { error: None }.render().unwrap())
}

#[instrument(skip(pool, email_service, form), fields(email))]
pub async fn signup(
    Extension(pool): Extension<SqlitePool>,
    Extension(email_service): Extension<Option<EmailService>>,
    Form(form): Form<EmailForm>,
) -> impl IntoResponse {
    let email = form.email.trim().to_lowercase();
    tracing::Span::current().record("email", &email.as_str());

    info!("Signup attempt initiated");

    // Validate email format
    if !email.contains('@') {
        return Html(
            SignupTemplate {
                error: Some("Invalid email address".to_string()),
            }
            .render()
            .unwrap(),
        )
        .into_response();
    }

    // Check if user already exists
    let exists = match database::user_exists(&pool, &email).await {
        Ok(e) => {
            if e {
                warn!(email = %email, "Signup attempt for existing user");
            }
            e
        }
        Err(_) => {
            error!("Database error checking user existence during signup");
            return Html(
                SignupTemplate {
                    error: Some("Database error".to_string()),
                }
                .render()
                .unwrap(),
            )
            .into_response()
        }
    };

    if exists {
        return Html(
            SignupTemplate {
                error: Some("Account already exists. Please log in.".to_string()),
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
                warn!(email = %email, "Rate limit exceeded for signup");
            }
            a
        }
        Err(_) => {
            error!("Database error checking rate limit during signup");
            return Html(
                SignupTemplate {
                    error: Some("Database error".to_string()),
                }
                .render()
                .unwrap(),
            )
            .into_response()
        }
    };

    if !allowed {
        return Html(
            SignupTemplate {
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
    match database::create_token(&pool, &email, &code, "signup").await {
        Ok(_) => {
            info!(email = %email, "Signup token created successfully");
        }
        Err(e) => {
            error!(error = ?e, "Failed to create signup token");
            return Html(
                SignupTemplate {
                    error: Some("Failed to create signup code".to_string()),
                }
                .render()
                .unwrap(),
            )
            .into_response();
        }
    };

    // Send email (don't fail if email service is not configured)
    if let Some(service) = email_service {
        match service.send_signup_code(&email, &code).await {
            Ok(_) => info!(email = %email, "Signup code email sent successfully"),
            Err(e) => {
                error!(email = %email, error = %e, "Failed to send signup code email");
                return Html(
                    SignupTemplate {
                        error: Some("Unable to send verification email. Please try again later.".to_string()),
                    }
                    .render()
                    .unwrap(),
                )
                .into_response();
            }
        }
    } else {
        info!(email = %email, code = %code, "[DEV] Signup code (email not configured)");
    }

    Html(
        VerifyTemplate {
            email,
            purpose: "signup".to_string(),
            error: None,
        }
        .render()
        .unwrap(),
    )
    .into_response()
}

#[instrument(skip(pool, form), fields(email, purpose))]
pub async fn verify(
    Extension(pool): Extension<SqlitePool>,
    Form(form): Form<VerifyForm>,
) -> impl IntoResponse {
    let email = form.email.trim().to_lowercase();
    let code = form.code.trim();

    tracing::Span::current()
        .record("email", &email.as_str())
        .record("purpose", &form.purpose.as_str());

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
                    email: email.clone(),
                    purpose: form.purpose.clone(),
                    error: Some("Database error".to_string()),
                }
                .render()
                .unwrap(),
            )
            .into_response()
        }
    };

    if !valid {
        return Html(
            VerifyTemplate {
                email,
                purpose: form.purpose,
                error: Some("Invalid or expired code".to_string()),
            }
            .render()
            .unwrap(),
        )
        .into_response();
    }

    match form.purpose.as_str() {
        "signup" => {
            // For signup, go to name registration page
            Html(
                RegisterNameTemplate {
                    email,
                    error: None,
                }
                .render()
                .unwrap(),
            )
            .into_response()
        }
        "login" => {
            // For login, check if user needs to set name
            let needs_name = match database::user_needs_name(&pool, &email).await {
                Ok(n) => n,
                Err(_) => {
                    return Html(
                        VerifyTemplate {
                            email,
                            purpose: "login".to_string(),
                            error: Some("Database error".to_string()),
                        }
                        .render()
                        .unwrap(),
                    )
                    .into_response()
                }
            };

            if needs_name {
                Html(
                    RegisterNameTemplate {
                        email,
                        error: None,
                    }
                    .render()
                    .unwrap(),
                )
                .into_response()
            } else {
                // Complete login - create session
                let (user_id, _user_name) = match database::get_user_by_email(&pool, &email).await {
                    Ok(Some((id, name))) => (id, name),
                    Ok(None) => {
                        return Html(
                            VerifyTemplate {
                                email: email.clone(),
                                purpose: "login".to_string(),
                                error: Some("Database error".to_string()),
                            }
                            .render()
                            .unwrap(),
                        )
                        .into_response();
                    }
                    Err(_) => {
                        return Html(
                            VerifyTemplate {
                                email: email.clone(),
                                purpose: "login".to_string(),
                                error: Some("Database error".to_string()),
                            }
                            .render()
                            .unwrap(),
                        )
                        .into_response();
                    }
                };

                // Create session
                let token = session::generate_session_token();
                let max_age_days = session::SESSION_MAX_AGE_DAYS;
                match database::create_session(&pool, user_id, &token, max_age_days).await {
                    Ok(_) => {
                        info!(user_id, email = %email, "User logged in successfully");
                        
                        // Build response with session cookie
                        let mut response = Redirect::to("/").into_response();
                        if let Some(cookie) = session::build_session_cookie(&token) {
                            response.headers_mut().insert(header::SET_COOKIE, cookie);
                        }
                        response
                    }
                    Err(e) => {
                        error!(error = ?e, "Failed to create session");
                        Html(
                            VerifyTemplate {
                                email: email.clone(),
                                purpose: "login".to_string(),
                                error: Some("Database error".to_string()),
                            }
                            .render()
                            .unwrap(),
                        )
                        .into_response()
                    }
                }
            }
        }
        _ => Redirect::to("/").into_response(),
    }
}

#[instrument(skip(pool, form), fields(email, name_len))]
pub async fn register_name(
    Extension(pool): Extension<SqlitePool>,
    Form(form): Form<NameForm>,
) -> impl IntoResponse {
    let email = form.email.trim().to_lowercase();
    let name = form.name.trim();

    tracing::Span::current()
        .record("email", &email.as_str())
        .record("name_len", name.len());

    info!("Name registration initiated");

    if name.is_empty() {
        warn!("Empty name submitted");
        return Html(
            RegisterNameTemplate {
                email,
                error: Some("Name is required".to_string()),
            }
            .render()
            .unwrap(),
        )
        .into_response();
    }

    // Check if user exists
    let exists = match database::user_exists(&pool, &email).await {
        Ok(e) => {
            debug!(user_exists = e, "User existence check");
            e
        }
        Err(_) => {
            error!("Database error checking user existence during name registration");
            return Html(
                RegisterNameTemplate {
                    email,
                    error: Some("Database error".to_string()),
                }
                .render()
                .unwrap(),
            )
            .into_response()
        }
    };

    let user_id = if exists {
        // Update existing user with name
        match database::update_user_name(&pool, &email, name).await {
            Ok(_) => {
                // Get user id
                match database::get_user_id_by_email(&pool, &email).await {
                    Ok(Some(id)) => id,
                    _ => {
                        return Html(
                            RegisterNameTemplate {
                                email,
                                error: Some("Database error".to_string()),
                            }
                            .render()
                            .unwrap(),
                        )
                        .into_response()
                    }
                }
            }
            Err(_) => {
                return Html(
                    RegisterNameTemplate {
                        email,
                        error: Some("Database error".to_string()),
                    }
                    .render()
                    .unwrap(),
                )
                .into_response()
            }
        }
    } else {
        // Create new user
        match database::create_user(&pool, &email, name).await {
            Ok(id) => id,
            Err(_) => {
                return Html(
                    RegisterNameTemplate {
                        email,
                        error: Some("Database error".to_string()),
                    }
                    .render()
                    .unwrap(),
                )
                .into_response()
            }
        }
    };

    // Create session
    let token = session::generate_session_token();
    let max_age_days = session::SESSION_MAX_AGE_DAYS;
    match database::create_session(&pool, user_id, &token, max_age_days).await {
        Ok(_) => {
            info!(user_id, email = %email, "User registered and logged in successfully");
            
            // Build response with session cookie
            let mut response = Redirect::to("/").into_response();
            if let Some(cookie) = session::build_session_cookie(&token) {
                response.headers_mut().insert(header::SET_COOKIE, cookie);
            }
            response
        }
        Err(e) => {
            error!(error = ?e, "Failed to create session");
            Html(
                RegisterNameTemplate {
                    email,
                    error: Some("Database error".to_string()),
                }
                .render()
                .unwrap(),
            )
            .into_response()
        }
    }
}

#[instrument(skip(session))]
pub async fn logout(
    Extension(pool): Extension<SqlitePool>,
    session: Session,
) -> impl IntoResponse {
    // Delete session from database if exists
    if let Some(token) = session.token {
        if let Err(e) = database::delete_session(&pool, &token).await {
            error!(error = ?e, "Failed to delete session");
        }
    }
    
    if let Some(user_id) = session.user_id {
        info!(user_id, "User logged out");
    }
    
    // Build response with cleared cookie
    let mut response = Redirect::to("/").into_response();
    response.headers_mut().insert(header::SET_COOKIE, session::clear_session_cookie());
    response
}
