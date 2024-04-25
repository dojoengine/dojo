CREATE TABLE event_messages (
    id TEXT NOT NULL PRIMARY KEY,
    keys TEXT,
    event_id TEXT NOT NULL,
    executed_at DATETIME NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_event_messages_keys ON event_messages (keys);
CREATE INDEX idx_event_messages_event_id ON event_messages (event_id);

CREATE TABLE event_model (
    entity_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    UNIQUE (entity_id, model_id),
    FOREIGN KEY (entity_id) REFERENCES event_messages (id),
    FOREIGN KEY (model_id) REFERENCES models (id)
);
CREATE INDEX idx_event_model_event_id ON event_model (entity_id);
CREATE INDEX idx_event_model_model_id ON event_model (model_id);