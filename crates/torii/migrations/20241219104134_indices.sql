-- Add index on the updated_at column of the entities table. As we are now allowing querying entities by their updated_at
-- we need to ensure that the updated_at column is indexed.
CREATE INDEX idx_entities_updated_at ON entities (updated_at);
