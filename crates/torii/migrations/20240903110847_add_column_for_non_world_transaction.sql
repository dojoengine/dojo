-- Rename pending_block_tx to last_pending_block_world_tx
ALTER TABLE contracts RENAME COLUMN pending_block_tx TO last_pending_block_world_tx;

-- Add new column last_pending_block_tx
ALTER TABLE contracts ADD COLUMN last_pending_block_tx TEXT;
