INSERT INTO components (id, name, address, class_hash, transaction_hash, storage_definition)
VALUES ('component_1', 'Game', '0x0', '0x0', '0x0', 
    '[{"name":"is_finished","type":"Boolean","slot":0,"offset":0}]');

INSERT INTO components (id, name, address, class_hash, transaction_hash, storage_definition)
VALUES ('component_2', 'Stats', '0x0', '0x0', '0x0', 
    '[{"name":"health","type":"u8","slot":0,"offset":0},{"name":"mana","type":"u8","slot":0,"offset":0}]');

INSERT INTO components (id, name, address, class_hash, transaction_hash, storage_definition)
VALUES ('component_3', 'Cash', '0x0', '0x0', '0x0', 
    '[{"name":"amount","type":"u32","slot":0,"offset":0}]');