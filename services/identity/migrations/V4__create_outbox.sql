CREATE TABLE outbox (
  id UUID PRIMARY KEY,
  aggregate_type TEXT NOT NULL,
  aggregate_id UUID NOT NULL,
  event_type TEXT NOT NULL,
  payload BYTEA NOT NULL,
  trace_id TEXT,
  span_id TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  published_at TIMESTAMPTZ
);

-- Publisher polls: SELECT ... WHERE published_at IS NULL ORDER BY created_at ASC LIMIT N FOR UPDATE SKIP LOCKED
CREATE INDEX idx_outbox_unpublished ON outbox (created_at ASC)
WHERE
  published_at IS NULL;
