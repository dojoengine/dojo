-- Create a new table to store the transaction and contract addresses
CREATE TABLE IF NOT EXISTS transaction_calls (
    transaction_hash TEXT NOT NULL,
    contract_address TEXT NOT NULL,
    entry_point_selector TEXT NOT NULL,
    calldata TEXT NOT NULL,
    call_type TEXT NOT NULL DEFAULT 'EXECUTE',
    caller_address TEXT NOT NULL,
    FOREIGN KEY (transaction_hash) REFERENCES transactions(id)
);

CREATE INDEX IF NOT EXISTS idx_transaction_calls_transaction_hash ON transaction_calls (transaction_hash);
CREATE INDEX IF NOT EXISTS idx_transaction_calls_contract_address ON transaction_calls (contract_address);
CREATE INDEX IF NOT EXISTS idx_transaction_calls_entry_point_selector ON transaction_calls (entry_point_selector);
CREATE INDEX IF NOT EXISTS idx_transaction_calls_caller_address ON transaction_calls (caller_address);