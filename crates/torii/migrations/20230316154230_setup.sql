CREATE TABLE indexer (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    head BIGINT NOT NULL DEFAULT 0
);

INSERT INTO indexer (head) VALUES (0);

CREATE TABLE components (
    id TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    address TEXT NOT NULL,
    class_hash TEXT NOT NULL,
    transaction_hash TEXT NOT NULL,
    storage_schema TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_components_created_at ON components (created_at);

CREATE TABLE system_calls (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    data TEXT,
    transaction_hash TEXT NOT NULL,
    system_id TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (system_id) REFERENCES systems(id)
);  

CREATE INDEX idx_system_calls_created_at ON system_calls (created_at);

CREATE TABLE systems (
    id TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    address TEXT NOT NULL,
    class_hash TEXT NOT NULL,
    transaction_hash TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_systems_created_at ON systems (created_at);

CREATE TABLE entities (
    id TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    transaction_hash TEXT NOT NULL,
    partition_id TEXT NOT NULL,
    keys TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_entities_partition_id ON entities (partition_id);
CREATE INDEX idx_entities_keys ON entities (keys);
CREATE INDEX idx_entities_keys_create_on ON entities (keys, created_at);

CREATE TABLE events (
    id TEXT NOT NULL PRIMARY KEY,
    system_call_id INTEGER NOT NULL,
    keys TEXT NOT NULL,
    data TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (system_call_id) REFERENCES system_calls(id)
);

CREATE INDEX idx_events_keys ON events (keys);
CREATE INDEX idx_events_created_at ON events (created_at);