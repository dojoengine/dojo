INSERT INTO components (id, name, address, class_hash, transaction_hash, storage_definition)
VALUES ('component_1', 'Game', '0x0', '0x0', '0x0', 
    '[{"name":"name","type":"FieldElement","slot":0,"offset":0},{"name":"is_finished","type":"Boolean","slot":0,"offset":0}]');

INSERT INTO components (id, name, address, class_hash, transaction_hash, storage_definition)
VALUES ('component_2', 'Stats', '0x0', '0x0', '0x0', 
    '[{"name":"health","type":"u8","slot":0,"offset":0},{"name":"mana","type":"u8","slot":0,"offset":0}]');


CREATE TABLE storage_game (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    is_finished BOOLEAN NOT NULL,
    version TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    component_id TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (entity_id) REFERENCES entities(id),
    FOREIGN KEY (component_id) REFERENCES components(id)
);
CREATE TABLE storage_stats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    health INTEGER NOT NULL,
    mana INTEGER NOT NULL,
    version TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    component_id TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (entity_id) REFERENCES entities(id),
    FOREIGN KEY (component_id) REFERENCES components(id)
);

INSERT INTO storage_game (id, name, is_finished, version, entity_id, component_id, created_at)
VALUES (1, '0x594F4C4F', 0, '0.0.0', 'entity_1', 'component_1', '2023-05-19T21:04:04Z');
INSERT INTO storage_stats (id, health, mana, version, entity_id, component_id, created_at)
VALUES (1, 42, 69, '0.0.0', 'entity_2', 'component_2', '2023-05-19T21:05:44Z');

