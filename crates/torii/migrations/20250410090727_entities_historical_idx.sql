-- Add indices on the entities_historical table.
CREATE INDEX idx_entities_historical_id ON entities_historical (id);
CREATE INDEX idx_entities_historical_keys ON entities_historical (keys);
CREATE INDEX idx_entities_historical_event_id ON entities_historical (event_id);
CREATE INDEX idx_entities_historical_executed_at ON entities_historical (executed_at);