use crate::database;
use axum::{
    extract::FromRequestParts,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use rand::Rng;
use sqlx::SqlitePool;

/// Session expiry: 2 years (in days)
pub const SESSION_MAX_AGE_DAYS: i64 = 730;

/// Session data extracted from the request
#[derive(Clone, Debug)]
pub struct Session {
    pub user_id: Option<i64>,
    pub user_name: Option<String>,
    pub email: Option<String>,
    pub token: Option<String>,
}

impl Session {
    pub fn is_authenticated(&self) -> bool {
        self.user_id.is_some()
    }
}

/// Extract session from request cookies
impl<S> FromRequestParts<S> for Session
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        // Get the pool from extensions
        let pool = parts
            .extensions
            .get::<SqlitePool>()
            .cloned()
            .ok_or_else(|| {
                (StatusCode::INTERNAL_SERVER_ERROR, "Database not available").into_response()
            })?;

        // Extract token from cookie
        let token = extract_session_token(&parts.headers);

        if let Some(token) = token {
            // Validate token in database
            match database::validate_session(&pool, &token).await {
                Ok(Some((user_id, user_name, email))) => Ok(Session {
                    user_id: Some(user_id),
                    user_name,
                    email: Some(email),
                    token: Some(token),
                }),
                Ok(None) => {
                    // Token not found or expired
                    Ok(Session {
                        user_id: None,
                        user_name: None,
                        email: None,
                        token: None,
                    })
                }
                Err(_) => {
                    // Database error - treat as unauthenticated
                    Ok(Session {
                        user_id: None,
                        user_name: None,
                        email: None,
                        token: None,
                    })
                }
            }
        } else {
            // No token in cookies
            Ok(Session {
                user_id: None,
                user_name: None,
                email: None,
                token: None,
            })
        }
    }
}

/// Generate a random session token
pub fn generate_session_token() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-";
    const TOKEN_LEN: usize = 32;

    let mut rng = rand::thread_rng();
    (0..TOKEN_LEN)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// Extract session token from cookie header
pub fn extract_session_token(headers: &HeaderMap) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;

    for cookie in cookie_header.split(';') {
        let trimmed = cookie.trim();
        if let Some(rest) = trimmed.strip_prefix("session_token=") {
            return Some(rest.to_string());
        }
    }

    None
}

/// Build a session cookie header value
pub fn build_session_cookie(token: &str) -> Option<HeaderValue> {
    let max_age_seconds = SESSION_MAX_AGE_DAYS * 24 * 60 * 60;

    HeaderValue::from_str(&format!(
        "session_token={token}; HttpOnly; Path=/; SameSite=Lax; Max-Age={max_age_seconds}"
    ))
    .ok()
}

/// Build a clear session cookie (for logout)
pub fn clear_session_cookie() -> HeaderValue {
    HeaderValue::from_static("session_token=; Max-Age=0; Path=/; HttpOnly; SameSite=Lax")
}
