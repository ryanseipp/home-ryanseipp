CREATE TABLE email_verification_tokens (
  id UUID PRIMARY KEY,
  user_id UUID NOT NULL REFERENCES users (id),
  token_hash BYTEA NOT NULL,
  expires_at TIMESTAMPTZ NOT NULL,
  consumed_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_verification_tokens_user_id ON email_verification_tokens (user_id);

-- Only unconsumed tokens need to be looked up; prevents reuse while keeping history
CREATE UNIQUE INDEX idx_verification_tokens_hash ON email_verification_tokens (token_hash)
WHERE
  consumed_at IS NULL;
