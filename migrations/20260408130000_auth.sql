-- Create users table
CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    email TEXT NOT NULL UNIQUE,
    name TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create email_tokens table for sign-in verification
CREATE TABLE IF NOT EXISTS email_tokens (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    email TEXT NOT NULL,
    code TEXT NOT NULL,
    expires_at DATETIME NOT NULL,
    used_at DATETIME,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    attempts INTEGER NOT NULL DEFAULT 0
);

-- Index for rate limiting queries
CREATE INDEX idx_email_tokens_email_created ON email_tokens(email, created_at);
CREATE INDEX idx_email_tokens_code ON email_tokens(code);
CREATE INDEX idx_users_email ON users(email);
