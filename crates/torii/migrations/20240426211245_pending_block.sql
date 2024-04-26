-- Add the pending block txn cursor to indexers table

CREATE TABLE indexers_new (
    id TEXT PRIMARY KEY NOT NULL,
    head BIGINT NOT NULL DEFAULT 0,
    pending_block_tx TEXT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Copy from old indexers
INSERT INTO indexers_new (id, head, pending_block_tx, created_at)
SELECT id, head, NULL, created_at
FROM indexers;

-- Disable foreign keys constraint so we can delete indexers
PRAGMA foreign_keys = OFF;

-- Drop old indexers
DROP TABLE indexers;

-- Rename table and recreate indexes
ALTER TABLE indexers_new RENAME TO indexers;

-- Renable foreign keys
PRAGMA foreign_keys = ON;