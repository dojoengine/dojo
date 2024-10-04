-- Add last_block_timestamp column for TPS calculation
ALTER TABLE contracts ADD COLUMN last_block_timestamp INTEGER;