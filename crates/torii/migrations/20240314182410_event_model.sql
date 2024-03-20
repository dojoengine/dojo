-- NOTE: sqlite does not support deleteing columns. Workaround is to create new table, copy, and delete old.

CREATE TABLE event_messages (
    id TEXT NOT NULL PRIMARY KEY,
    keys TEXT,
    event_id TEXT NOT NULL,
    model_names TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_event_messages_keys ON event_messages (keys);

CREATE INDEX idx_event_messages_event_id ON event_messages (event_id);

-- Create new table without model_names column
CREATE TABLE event_messages_new (
    id TEXT NOT NULL PRIMARY KEY,
    keys TEXT,
    event_id TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Copy from old event_messages
INSERT INTO event_messages_new (id, keys, event_id, created_at, updated_at)
SELECT id, keys, event_id, created_at, updated_at
FROM event_messages;

-- Disable foreign keys constraint so we can delete event_messages
PRAGMA foreign_keys = OFF;

-- Drop old event_messages
DROP TABLE event_messages;

-- Rename table and recreate indexes
ALTER TABLE event_messages_new RENAME TO event_messages;
CREATE INDEX idx_event_messages_keys ON event_messages (keys);
CREATE INDEX idx_event_messages_event_id ON event_messages (event_id);

-- Renable foreign keys
PRAGMA foreign_keys = ON;

-- New table to track event to model relationships
CREATE TABLE event_model (
    entity_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    UNIQUE (entity_id, model_id),
    FOREIGN KEY (entity_id) REFERENCES event_messages (id),
    FOREIGN KEY (model_id) REFERENCES models (id)
);
CREATE INDEX idx_event_model_event_id ON event_model (entity_id);
CREATE INDEX idx_event_model_model_id ON event_model (model_id);