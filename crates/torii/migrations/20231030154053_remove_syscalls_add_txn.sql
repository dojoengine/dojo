DROP TABLE system_calls;

CREATE TABLE transactions (
    id TEXT NOT NULL PRIMARY KEY,
    transaction_hash TEXT NOT NULL,
    sender_address TEXT NOT NULL,
    calldata TEXT NOT NULL,
    max_fee TEXT NOT NULL,
    signature TEXT NOT NULL,
    nonce TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (transaction_hash)
);

CREATE TABLE transaction_receipts (
    id TEXT NOT NULL PRIMARY KEY,
    transaction_hash TEXT NOT NULL,
    actual_fee TEXT NOT NULL,
    finality_status TEXT NOT NULL,
    block_hash TEXT NOT NULL,
    block_number INTEGER NOT NULL,
    execution_result TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (transaction_hash)
);