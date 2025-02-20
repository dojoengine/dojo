CREATE TABLE IF NOT EXISTS transaction_contract (
    transaction_hash TEXT NOT NULL,
    contract_address TEXT NOT NULL,
    UNIQUE (transaction_hash, contract_address),
    FOREIGN KEY (transaction_hash) REFERENCES transactions(id),
    FOREIGN KEY (contract_address) REFERENCES contracts(id)
);

CREATE INDEX IF NOT EXISTS idx_transaction_contract_transaction_hash ON transaction_contract (transaction_hash);
CREATE INDEX IF NOT EXISTS idx_transaction_contract_contract_address ON transaction_contract (contract_address);