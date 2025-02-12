-- NOTE: sqlite does not support deleting columns. Workaround is to create new table, copy, and delete old.

-- Create new table without executor_address and executor_class_hash columns
CREATE TABLE worlds_new (
    id TEXT PRIMARY KEY NOT NULL,
    world_address TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Copy from old worlds
INSERT INTO worlds_new (id, world_address, created_at)
SELECT id, world_address, created_at
FROM worlds;

-- Disable foreign keys constraint so we can delete worlds
PRAGMA foreign_keys = OFF;

-- Drop old worlds
DROP TABLE worlds;

-- Rename table and recreate indexes
ALTER TABLE worlds_new RENAME TO worlds;

-- Renable foreign keys
PRAGMA foreign_keys = ON;
