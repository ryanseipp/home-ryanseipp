CREATE TABLE users (
  id UUID PRIMARY KEY,
  username TEXT NOT NULL,
  email TEXT NOT NULL,
  given_name TEXT NOT NULL,
  family_name TEXT NOT NULL,
  password_hash TEXT NOT NULL,
  email_verified BOOLEAN NOT NULL DEFAULT FALSE,
  status TEXT NOT NULL DEFAULT 'pending_verification',
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  deleted_at TIMESTAMPTZ
);

-- Case-insensitive uniqueness, excluding soft-deleted rows
CREATE UNIQUE INDEX idx_users_username ON users (LOWER(username))
WHERE
  deleted_at IS NULL;

CREATE UNIQUE INDEX idx_users_email ON users (LOWER(email))
WHERE
  deleted_at IS NULL;

CREATE INDEX idx_users_status ON users (status)
WHERE
  deleted_at IS NULL;
