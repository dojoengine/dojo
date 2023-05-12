-- Add migration script here
ALTER TABLE entities ADD COLUMN partition_id TEXT NOT NULL;
ALTER TABLE entities ADD COLUMN keys TEXT NOT NULL;
ALTER TABLE entities ADD COLUMN created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP;
ALTER TABLE entities ADD COLUMN updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP;

CREATE INDEX idx_entities_partition_id ON entities (partition_id);
CREATE INDEX idx_entities_keys ON entities (keys);