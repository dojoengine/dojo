CREATE TABLE events (
    id TEXT NOT NULL PRIMARY KEY,
    system_call_id INTEGER NOT NULL,
    keys TEXT NOT NULL,
    data TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (system_call_id) REFERENCES system_calls(id)
);

CREATE INDEX idx_events_keys ON events (keys);