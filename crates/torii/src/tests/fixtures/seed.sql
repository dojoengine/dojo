/* seed db with mock data, spawning a game and two players */ 

/* register components and systems */ 
INSERT INTO components (id, name, properties, address, class_hash, transaction_hash) VALUES ('component_1', 'Game', NULL, '0x0', '0x0', '0x0');
INSERT INTO components (id, name, properties, address, class_hash, transaction_hash) VALUES ('component_2', 'Stats', NULL, '0x0', '0x0', '0x0');
INSERT INTO components (id, name, properties, address, class_hash, transaction_hash) VALUES ('component_3', 'Cash', NULL, '0x0', '0x0', '0x0');
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
INSERT INTO entity_states (entity_id, component_id, data) 
VALUES ('entity_1', 'component_1', '{\"game\": {\"start_time\": \"20:00\", \"max_players\": 2, \"is_finished\": false}}');
INSERT INTO entity_states (entity_id, component_id, data) VALUES ('entity_2', 'component_2', '{\"stats\": {\"health\": 100}}');
INSERT INTO entity_states (entity_id, component_id, data) VALUES ('entity_2', 'component_3', '{\"cash\": {\"amount\": 100}}');
INSERT INTO entity_states (entity_id, component_id, data) VALUES ('entity_3', 'component_2', '{\"stats\": {\"health\": 100}}');
INSERT INTO entity_states (entity_id, component_id, data) VALUES ('entity_3', 'component_3', '{\"cash\": {\"amount\": 100}}');

