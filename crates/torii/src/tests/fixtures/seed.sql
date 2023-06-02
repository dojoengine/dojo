/* seed db with mock data, spawning a game and two players */ 
INSERT INTO indexer (head) VALUES (0);

/* register components and systems */ 
INSERT INTO components (id, name, address, class_hash, transaction_hash, storage_definition)
VALUES ('component_1', 'Game', '0x0', '0x0', '0x0', 
    '[{"name":"name","type":"FieldElement","slot":0,"offset":0},{"name":"is_finished","type":"Boolean","slot":0,"offset":0}]');
INSERT INTO components (id, name, address, class_hash, transaction_hash, storage_definition)
VALUES ('component_2', 'Stats', '0x0', '0x0', '0x0', 
    '[{"name":"health","type":"u8","slot":0,"offset":0},{"name":"mana","type":"u8","slot":0,"offset":0}]');
INSERT INTO components (id, name, address, class_hash, transaction_hash, storage_definition)
VALUES ('component_3', 'Cash', '0x0', '0x0', '0x0', 
    '[{"name":"amount","type":"u32","slot":0,"offset":0}]');
INSERT INTO systems (id, name, address, class_hash, transaction_hash) VALUES ('system_1', 'SpawnGame', '0x0', '0x0', '0x0');
INSERT INTO systems (id, name, address, class_hash, transaction_hash) VALUES ('system_2', 'SpawnPlayer', '0x0', '0x0', '0x0');
INSERT INTO systems (id, name, address, class_hash, transaction_hash) VALUES ('system_3', 'SpawnPlayer', '0x0', '0x0', '0x0');

/* system calls to spawn game and player */
INSERT INTO system_calls (id, system_id, transaction_hash, data) VALUES (1, 'system_1', '0x0', 'game_data');
INSERT INTO system_calls (id, system_id, transaction_hash, data) VALUES (2, 'system_2', '0x0', 'player_data');
INSERT INTO system_calls (id, system_id, transaction_hash, data) VALUES (3, 'system_3', '0x0', 'player_data');

/* events and entities */ 
INSERT INTO events (id, system_call_id, keys, data, created_at) 
VALUES ('event_1', 1, 'GameSpawned', '{\"game_id\": \"game_1\"}', '2023-05-19T20:29:53Z');
INSERT INTO events (id, system_call_id, keys, data, created_at) 
VALUES ('event_2', 2, 'PlayerSpawned', '{\"player_id\": \"player_1\"}', '2023-05-19T20:45:28Z');
INSERT INTO events (id, system_call_id, keys, data, created_at) 
VALUES ('event_3', 3, 'PlayerSpawned', '{\"player_id\": \"player_2\"}', '2023-05-19T20:50:01Z');
INSERT INTO events (id, system_call_id, keys, data, created_at) 
VALUES ('event_4', 4, 'LocationSpawned', '{\"location_id\": \"location_1\"}', '2023-05-19T21:04:04Z');
INSERT INTO events (id, system_call_id, keys, data, created_at) 
VALUES ('event_5', 5, 'LocationSpawned', '{\"location_id\": \"location_2\"}', '2023-05-19T21:10:33Z');
INSERT INTO events (id, system_call_id, keys, data, created_at) 
VALUES ('event_6', 6, 'LocationSpawned', '{\"location_id\": \"location_3\"}', '2023-05-19T21:11:28Z');
INSERT INTO entities (id, name, partition_id, keys, transaction_hash, created_at ) 
VALUES ( 'entity_1', 'Game', 'game_1', '', '0x0', '2023-05-19T21:04:04Z');
INSERT INTO entities (id, name, partition_id, keys, transaction_hash, created_at ) 
VALUES ( 'entity_2', 'Player', 'game_1', 'player_1', '0x0', '2023-05-19T21:05:44Z');
INSERT INTO entities (id, name, partition_id, keys, transaction_hash, created_at ) 
VALUES ( 'entity_3', 'Player', 'game_1', 'player_2', '0x0', '2023-05-19T21:08:12Z');

/* tables for component storage, created at runtime by processor */
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
CREATE TABLE storage_cash (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    amount INTEGER NOT NULL,
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
VALUES (1, 100, 100, '0.0.0', 'entity_2', 'component_2', '2023-05-19T21:05:44Z');
INSERT INTO storage_stats (id, health, mana, version, entity_id, component_id, created_at)
VALUES (2, 50, 50, '0.0.0', 'entity_3', 'component_2', '2023-05-19T21:08:12Z');
INSERT INTO storage_cash (id, amount, version, entity_id, component_id, created_at)
VALUES (1, 77, '0.0.0', 'entity_2', 'component_3', '2023-05-19T21:05:44Z');
INSERT INTO storage_cash (id, amount, version, entity_id, component_id, created_at)
VALUES (2, 88, '0.0.0', 'entity_3', 'component_3', '2023-05-19T21:08:12Z');
