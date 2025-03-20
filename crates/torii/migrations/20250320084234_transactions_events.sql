-- Add a foreign key to transactions table from events table
ALTER TABLE events ADD CONSTRAINT fk_events_transactions FOREIGN KEY (transaction_hash) REFERENCES transactions(id);

-- Remove the id column from the transactions table and set transaction_hash as primary key
ALTER TABLE transactions DROP COLUMN id;
ALTER TABLE transactions ADD PRIMARY KEY (transaction_hash);