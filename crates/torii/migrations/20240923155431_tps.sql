-- Add tps related columns
ALTER TABLE contracts ADD COLUMN tps INTEGER;
ALTER TABLE contracts ADD COLUMN last_block_timestamp INTEGER;