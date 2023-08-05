CREATE TABLE indexers (
    id TEXT PRIMARY KEY NOT NULL,
    head BIGINT NOT NULL DEFAULT 0,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE worlds (
    id TEXT PRIMARY KEY NOT NULL,
    world_address TEXT NOT NULL,
    world_class_hash TEXT,
    executor_address TEXT,
    executor_class_hash TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE components (
    id TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    class_hash TEXT NOT NULL,
    transaction_hash TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_components_created_at ON components (created_at);

CREATE TABLE component_members(
    component_id TEXT NOT NULL,
    name TEXT NOT NULL,
    type TEXT NOT NULL,
    key BOOLEAN NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (component_id, name),
    FOREIGN KEY (component_id) REFERENCES components(id)
);

CREATE TABLE system_calls (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    data TEXT NOT NULL,
    transaction_hash TEXT NOT NULL,
    system_id TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (system_id) REFERENCES systems(id)
);  

CREATE INDEX idx_system_calls_created_at ON system_calls (created_at);

CREATE TABLE systems (
    id TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    class_hash TEXT NOT NULL,
    transaction_hash TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_systems_created_at ON systems (created_at);

CREATE TABLE entities (
    id TEXT NOT NULL PRIMARY KEY,
    keys TEXT,
    component_names TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

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