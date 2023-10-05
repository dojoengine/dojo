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

CREATE TABLE models (
    id TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    layout BLOB NOT NULL,
    transaction_hash TEXT,
    class_hash TEXT NOT NULL,
    packed_size INTEGER NOT NULL,
    unpacked_size INTEGER NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_models_created_at ON models (created_at);

CREATE TABLE model_members(
    id TEXT NOT NULL,
    model_idx INTEGER NOT NULL,
    member_idx INTEGER NOT NULL,
    model_id TEXT NOT NULL,
    name TEXT NOT NULL,
    type TEXT NOT NULL,
    type_enum TEXT DEFAULT 'Primitive' CHECK(type_enum IN ('Primitive', 'Struct', 'Enum', 'Tuple')) NOT NULL,
    key BOOLEAN NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id, member_idx) FOREIGN KEY (model_id) REFERENCES models(id)
);

CREATE INDEX idx_model_members_model_id ON model_members (model_id);

CREATE TABLE system_calls (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    data TEXT NOT NULL,
    transaction_hash TEXT NOT NULL,
    system_id TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (system_id) REFERENCES systems(id),
    UNIQUE (transaction_hash)
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
    event_id TEXT NOT NULL,
    model_names TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_entities_keys ON entities (keys);
CREATE INDEX idx_entities_event_id ON entities (event_id);

CREATE TABLE events (
    id TEXT NOT NULL PRIMARY KEY,
    keys TEXT NOT NULL,
    data TEXT NOT NULL,
    transaction_hash TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_events_keys ON events (keys);