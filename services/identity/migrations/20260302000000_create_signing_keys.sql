CREATE TABLE signing_keys (
  kid TEXT PRIMARY KEY,
  algorithm TEXT NOT NULL,
  encrypted_private_key BYTEA NOT NULL,
  public_jwk JSONB NOT NULL,
  status TEXT NOT NULL DEFAULT 'active',
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_signing_keys_status ON signing_keys (status);
