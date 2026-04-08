use sqlx::SqlitePool;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("Database error")]
    Sqlx(#[from] sqlx::Error),
}

// Token expiry: 15 minutes
const TOKEN_EXPIRY_MINUTES: i64 = 15;
// Rate limit: 3 tokens per hour per email
const RATE_LIMIT_COUNT: i64 = 3;
const RATE_LIMIT_HOURS: i64 = 1;

// Check rate limit: max 3 tokens per hour per email
pub async fn check_rate_limit(pool: &SqlitePool, email: &str) -> Result<bool, DbError> {
    use chrono::{Duration, Utc};
    
    let since = Utc::now() - Duration::hours(RATE_LIMIT_HOURS);

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM email_tokens WHERE email = ? AND created_at > ? AND used_at IS NULL"
    )
    .bind(email)
    .bind(since.naive_utc())
    .fetch_one(pool)
    .await?;

    Ok(count < RATE_LIMIT_COUNT)
}

// Create email token
pub async fn create_token(
    pool: &SqlitePool,
    email: &str,
    code: &str,
    purpose: &str,
) -> Result<(), DbError> {
    use chrono::{Duration, Utc};
    
    let expires_at = Utc::now() + Duration::minutes(TOKEN_EXPIRY_MINUTES);

    sqlx::query(
        "INSERT INTO email_tokens (email, code, purpose, expires_at) VALUES (?, ?, ?, ?)"
    )
    .bind(email)
    .bind(code)
    .bind(purpose)
    .bind(expires_at.naive_utc())
    .execute(pool)
    .await?;

    Ok(())
}

// Validate email token and mark as used if valid
pub async fn validate_and_use_token(
    pool: &SqlitePool,
    email: &str,
    code: &str,
) -> Result<bool, DbError> {
    use chrono::Utc;
    
    let now = Utc::now().naive_utc();

    let result: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM email_tokens WHERE email = ? AND code = ? AND used_at IS NULL AND expires_at > ?"
    )
    .bind(email)
    .bind(code)
    .bind(now)
    .fetch_optional(pool)
    .await?;

    if let Some((id,)) = result {
        // Mark as used
        sqlx::query("UPDATE email_tokens SET used_at = ? WHERE id = ?")
            .bind(now)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(true)
    } else {
        Ok(false)
    }
}

// Create user with name
pub async fn create_user(pool: &SqlitePool, email: &str, name: &str) -> Result<i64, DbError> {
    let id = sqlx::query("INSERT INTO users (email, name) VALUES (?, ?)")
        .bind(email)
        .bind(name)
        .execute(pool)
        .await?
        .last_insert_rowid();
    Ok(id)
}

// Update user name
pub async fn update_user_name(pool: &SqlitePool, email: &str, name: &str) -> Result<(), DbError> {
    sqlx::query("UPDATE users SET name = ? WHERE email = ?")
        .bind(name)
        .bind(email)
        .execute(pool)
        .await?;
    Ok(())
}

// Check if user exists
pub async fn user_exists(pool: &SqlitePool, email: &str) -> Result<bool, DbError> {
    let result: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM users WHERE email = ?").bind(email).fetch_optional(pool).await?;
    Ok(result.is_some())
}

// Check if user needs to set name
pub async fn user_needs_name(pool: &SqlitePool, email: &str) -> Result<bool, DbError> {
    let result: Option<(Option<String>,)> =
        sqlx::query_as("SELECT name FROM users WHERE email = ?").bind(email).fetch_optional(pool).await?;
    Ok(matches!(result, Some((None,))))
}

// Get user by email
pub async fn get_user_by_email(
    pool: &SqlitePool,
    email: &str,
) -> Result<Option<(i64, Option<String>)>, DbError> {
    let result: Option<(i64, Option<String>)> =
        sqlx::query_as("SELECT id, name FROM users WHERE email = ?")
            .bind(email)
            .fetch_optional(pool)
            .await?;
    Ok(result)
}

// Get user id by email
pub async fn get_user_id_by_email(pool: &SqlitePool, email: &str) -> Result<Option<i64>, DbError> {
    let result: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM users WHERE email = ?")
            .bind(email)
            .fetch_optional(pool)
            .await?;
    Ok(result.map(|(id,)| id))
}

// Session functions

/// Create a new session token for a user
pub async fn create_session(
    pool: &SqlitePool,
    user_id: i64,
    token: &str,
    max_age_days: i64,
) -> Result<(), DbError> {
    use chrono::{Duration, Utc};
    
    let expires_at = Utc::now() + Duration::days(max_age_days);

    sqlx::query(
        "INSERT INTO sessions (token, user_id, expires_at) VALUES (?, ?, ?)"
    )
    .bind(token)
    .bind(user_id)
    .bind(expires_at.naive_utc())
    .execute(pool)
    .await?;

    Ok(())
}

/// Validate a session token and return user info if valid
pub async fn validate_session(
    pool: &SqlitePool,
    token: &str,
) -> Result<Option<(i64, Option<String>)>, DbError> {
    use chrono::Utc;
    
    let now = Utc::now().naive_utc();

    let result: Option<(i64, Option<String>)> = sqlx::query_as(
        "SELECT s.user_id, u.name FROM sessions s 
         JOIN users u ON s.user_id = u.id 
         WHERE s.token = ? AND s.expires_at > ?"
    )
    .bind(token)
    .bind(now)
    .fetch_optional(pool)
    .await?;

    Ok(result)
}

/// Delete a session token (logout)
pub async fn delete_session(pool: &SqlitePool, token: &str) -> Result<(), DbError> {
    sqlx::query("DELETE FROM sessions WHERE token = ?")
        .bind(token)
        .execute(pool)
        .await?;
    Ok(())
}
