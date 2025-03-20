-- Add an index to the events table on transaction_hash
CREATE INDEX idx_events_transaction_hash ON events (transaction_hash);

-- Remove the id column from the transactions table and set transaction_hash as primary key
ALTER TABLE transactions DROP COLUMN id;
ALTER TABLE transactions ADD PRIMARY KEY (transaction_hash);