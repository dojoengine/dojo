/* seed db with mock data, spawning a game and two players */ 

/* register components and systems */ 
INSERT INTO components (id, name, class_hash, transaction_hash)
VALUES ('component_1', 'Game', '0x0', '0x0');
INSERT INTO components (id, name, class_hash, transaction_hash)
VALUES ('component_2', 'Stats', '0x0', '0x0');
INSERT INTO components (id, name, class_hash, transaction_hash)
VALUES ('component_3', 'Cash', '0x0', '0x0');
INSERT INTO systems (id, name, class_hash, transaction_hash) VALUES ('system_1', 'SpawnGame', '0x0', '0x0');
INSERT INTO systems (id, name, class_hash, transaction_hash) VALUES ('system_2', 'SpawnPlayer', '0x0', '0x0');
INSERT INTO systems (id, name, class_hash, transaction_hash) VALUES ('system_3', 'SpawnPlayer', '0x0', '0x0');


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
INSERT INTO entities (id, partition, keys, transaction_hash, created_at ) 
VALUES ( 'entity_1', 'game_1', '', '0x0', '2023-05-19T21:04:04Z');
INSERT INTO entities (id, partition, keys, transaction_hash, created_at ) 
VALUES ( 'entity_2', 'game_1', 'player_1', '0x0', '2023-05-19T21:05:44Z');
INSERT INTO entities (id, partition, keys, transaction_hash, created_at ) 
VALUES ( 'entity_3', 'game_1', 'player_2', '0x0', '2023-05-19T21:08:12Z');

INSERT INTO component_members (component_id, name, type, slot, offset )
VALUES ( 'component_1', 'is_finished', 'bool', 0, 0);
INSERT INTO component_members (component_id, name, type, slot, offset )
VALUES ( 'component_2', 'health', 'u8', 0, 0);
INSERT INTO component_members (component_id, name, type, slot, offset )
VALUES ( 'component_2', 'mana', 'u8', 0, 0);
INSERT INTO component_members (component_id, name, type, slot, offset )
VALUES ( 'component_3', 'amount', 'u32', 0, 0);

/* tables for component storage, created at runtime by processor */
CREATE TABLE external_game (
    id TEXT NOT NULL,
    partition TEXT NOT NULL,
    external_is_finished BOOLEAN NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE external_stats (
    id TEXT NOT NULL,
    partition TEXT NOT NULL,
    external_health TEXT NOT NULL,
    external_mana TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE external_cash (
    id TEXT NOT NULL,
    partition TEXT NOT NULL,
    external_amount TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO external_game (id, partition, external_is_finished, created_at)
VALUES ('1', 'game_partition', 0, '2023-05-19T21:04:04Z');
INSERT INTO external_stats (id, partition, external_health, external_mana, created_at)
VALUES ('2', 'game_partition', '69', '42', '2023-05-19T21:05:44Z');
INSERT INTO external_stats (id, partition, external_health, external_mana, created_at)
VALUES ('3', 'game_partition', '42', '69', '2023-05-19T21:08:12Z');
INSERT INTO external_cash (id, partition, external_amount, created_at)
VALUES ('2', 'game_partition', '88', '2023-05-19T21:05:44Z');
INSERT INTO external_cash (id, partition, external_amount, created_at)
VALUES ('3', 'game_partition', '66', '2023-05-19T21:08:12Z');
