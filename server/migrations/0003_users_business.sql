-- Individual user accounts, login sessions, who-did-what on orders and price
-- changes, and the business profile (letterhead, payment details, logo).

CREATE TABLE users (
    id             UUID PRIMARY KEY,
    username       TEXT        NOT NULL UNIQUE,
    display_name   TEXT        NOT NULL,
    role           TEXT        NOT NULL CHECK (role IN ('admin', 'staff')),
    active         BOOLEAN     NOT NULL DEFAULT TRUE,
    password_hash  TEXT        NOT NULL,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Only a hash of the login token is stored, so a database leak does not hand
-- out working logins.
CREATE TABLE auth_sessions (
    token_hash  TEXT        PRIMARY KEY,
    user_id     UUID        NOT NULL REFERENCES users (id),
    expires_at  TIMESTAMPTZ NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX auth_sessions_user ON auth_sessions (user_id);

ALTER TABLE orders         ADD COLUMN created_by TEXT;
ALTER TABLE rules_versions ADD COLUMN changed_by TEXT;

CREATE TABLE app_settings (
    key         TEXT        PRIMARY KEY,
    value       JSONB       NOT NULL,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_by  TEXT
);

CREATE TABLE app_assets (
    key           TEXT        PRIMARY KEY,
    content_type  TEXT        NOT NULL,
    data          BYTEA       NOT NULL,
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
