-- Add index on the updated_at column of the entities table. As we are now allowing querying entities by their updated_at
-- we need to ensure that the updated_at column is indexed.
CREATE INDEX idx_entities_updated_at ON entities (updated_at);

-- Add indices on the event_messages_historical table.
CREATE INDEX idx_event_messages_historical_id ON event_messages_historical (id);
CREATE INDEX idx_event_messages_historical_keys ON event_messages_historical (keys);
CREATE INDEX idx_event_messages_historical_event_id ON event_messages_historical (event_id);
CREATE INDEX idx_event_messages_historical_executed_at ON event_messages_historical (executed_at);