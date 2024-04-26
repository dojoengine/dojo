-- Add the pending block txn cursor to indexers table
ALTER TABLE indexers ADD COLUMN pending_block_tx TEXT NULL DEFAULT NULL;