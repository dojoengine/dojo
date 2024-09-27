CREATE TABLE balances (
    -- account_address:token_id
    id TEXT NOT NULL PRIMARY KEY,
    balance TEXT NOT NULL,
    account_address TEXT NOT NULL,
    contract_address TEXT NOT NULL,
    -- contract_address:token_id
    token_id TEXT NOT NULL,
    FOREIGN KEY (token_id) REFERENCES tokens(id)
);

CREATE INDEX balances_account_address ON balances (account_address);
CREATE INDEX balances_contract_address ON balances (contract_address);

CREATE TABLE tokens (
    -- contract_address:token_id
    id TEXT NOT NULL PRIMARY KEY,
    contract_address TEXT NOT NULL,
    name TEXT NOT NULL,
    symbol TEXT NOT NULL,
    decimals INTEGER NOT NULL,
    FOREIGN KEY (contract_address) REFERENCES contracts(id)
);

CREATE TABLE erc_transfers (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    contract_address TEXT NOT NULL,
    from_address TEXT NOT NULL,
    to_address TEXT NOT NULL,
    amount TEXT NOT NULL,
    -- contract_address:token_id
    token_id TEXT NOT NULL,
    executed_at DATETIME NOT NULL,
    FOREIGN KEY (token_id) REFERENCES tokens(id)
);
