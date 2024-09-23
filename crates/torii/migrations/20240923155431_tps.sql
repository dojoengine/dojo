-- Add tps column
ALTER TABLE contracts ADD COLUMN tps INTEGER;
-- Add last block timestamp column
ALTER TABLE contracts ADD COLUMN last_block_timestamp DATETIME;