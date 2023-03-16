CREATE TABLE components (
    id TEXT NOT NULL PRIMARY KEY,
    name TEXT,
    properties TEXT,
    address TEXT NOT NULL,
    class_hash TEXT NOT NULL,
    transaction_hash TEXT NOT NULL
);

CREATE TABLE system_calls (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    data TEXT,
    transaction_hash TEXT NOT NULL,
    system_id TEXT NOT NULL,
    FOREIGN KEY (system_id) REFERENCES systems(id)
);

CREATE TABLE systems (
    id TEXT NOT NULL PRIMARY KEY,
    name TEXT,
    address TEXT NOT NULL,
    class_hash TEXT NOT NULL,
    transaction_hash TEXT NOT NULL
);

CREATE TABLE entities (
    id TEXT NOT NULL PRIMARY KEY,
    name TEXT,
    transaction_hash TEXT NOT NULL
);

CREATE TABLE entity_state_updates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    entity_id TEXT NOT NULL,
    component_id TEXT NOT NULL,
    transaction_hash TEXT NOT NULL,
    data TEXT,
    FOREIGN KEY (entity_id) REFERENCES entities(id),
    FOREIGN KEY (component_id) REFERENCES components(id)
);
    
CREATE TABLE entity_states (
    entity_id TEXT NOT NULL,
    component_id TEXT NOT NULL,
    data TEXT,
    FOREIGN KEY (entity_id) REFERENCES entities(id),
    FOREIGN KEY (component_id) REFERENCES components(id),
    UNIQUE (entity_id, component_id)
);