-- NOTE: sqlite does not support deleteing columns. Workaround is to create new table, copy, and delete old.

-- Create new table without model_names column
CREATE TABLE entities_new (
    id TEXT NOT NULL PRIMARY KEY,
    keys TEXT,
    event_id TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Copy from old entities
INSERT INTO entities_new (id, keys, event_id, created_at, updated_at)
SELECT id, keys, event_id, created_at, updated_at
FROM entities;

-- Disable foreign keys constraint so we can delete entities
PRAGMA foreign_keys = OFF;

-- Drop old entities
DROP TABLE entities;

-- Rename table and recreate indexes
ALTER TABLE entities_new RENAME TO entities;
CREATE INDEX idx_entities_keys ON entities (keys);
CREATE INDEX idx_entities_event_id ON entities (event_id);

-- Renable foreign keys
PRAGMA foreign_keys = ON;

-- New table to track entity to model relationships
CREATE TABLE entity_model (
    entity_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    UNIQUE (entity_id, model_id),
    FOREIGN KEY (entity_id) REFERENCES entities (id),
    FOREIGN KEY (model_id) REFERENCES models (id)
);
CREATE INDEX idx_entity_model_entity_id ON entity_model (entity_id);
CREATE INDEX idx_entity_model_model_id ON entity_model (model_id);