INSERT INTO components (id, name, address, class_hash, transaction_hash, storage_schema)
VALUES ('component_1', 'Game', '0x0', '0x0', '0x0', 
    'type GameComponent { isFinished: Boolean! entity: Entity! component: Component! createdAt: DateTime! }');

INSERT INTO components (id, name, address, class_hash, transaction_hash, storage_schema)
VALUES ('component_2', 'Stats', '0x0', '0x0', '0x0', 
    'type StatsComponent { health: U8! entity: Entity! component: Component! createdAt: DateTime! }');

INSERT INTO components (id, name, address, class_hash, transaction_hash, storage_schema)
VALUES ('component_3', 'Cash', '0x0', '0x0', '0x0', 
    'type CashComponent { amount: U32! entity: Entity! component: Component! createdAt: DateTime! }');