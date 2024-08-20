CREATE TABLE contracts (
    -- contract_address
    id TEXT NOT NULL PRIMARY KEY,
    contract_address TEXT NOT NULL,
    -- "WORLD", "ERC20", etc...
    contract_type TEXT NOT NULL,
    head BIGINT,
    pending_block_tx TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Copy data from world and indexer tables into contracts table
INSERT INTO contracts (id, contract_address, contract_type, head, pending_block_tx)
SELECT 
    w.id,
    w.world_address,
    i.head,
    i.pending_block_tx,
    'WORLD'
FROM worlds w
LEFT JOIN indexers i ON w.id = i.id;

-- remove unused tables
DROP TABLE worlds;
DROP TABLE indexers;