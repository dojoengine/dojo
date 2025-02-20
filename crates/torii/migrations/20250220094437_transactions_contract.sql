CREATE TABLE IF NOT EXISTS transaction_contract (
    transaction_hash TEXT REFERENCES transactions(id),
    contract_address TEXT REFERENCES contracts(id),
    PRIMARY KEY (transaction_hash, contract_address)
);