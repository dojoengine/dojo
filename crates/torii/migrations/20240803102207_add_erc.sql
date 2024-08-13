CREATE TABLE erc20_balances (
    account_address TEXT NOT NULL,
    token_address TEXT NOT NULL,
    balance TEXT NOT NULL,
    PRIMARY KEY (account_address, token_address)
);

CREATE TABLE erc721_balances (
    account_address TEXT NOT NULL,
    token_address TEXT NOT NULL,
    token_id TEXT NOT NULL,
    PRIMARY KEY (account_address, token_address, token_id)
);

-- query: get all balances for a given account
-- query(in future): get all balanves for a given token contract

-- one row represents a token contract on chain
CREATE TABLE contracts (
    contract_address TEXT NOT NULL PRIMARY KEY,
    -- "ERC20" or "ERC721" or "ERC1155" or "WORLD"
    contract_type TEXT NOT NULL,
);

CREATE TABLE token_id (
    --! for ERC1155: both name and symbol are offchain (so would be null)
    name TEXT,
    symbol TEXT,
    --! Null for ERC721 and its part of metadata in ERC1155 (so needs to be fetched offchain)
    decimals TEXT,
    --! total_supply would in erc1155 would need to be a map of token_id to balance
    total_supply TEXT

)

CREATE TABLE erc_balances (
    -- for ERC20, this would be (account_address:token_address:0x0)
    -- for ERC721 and ERC1155, this would be (account_address:token_address:token_id)
    id TEXT NOT NULL PRIMARY KEY,
    account_address TEXT NOT NULL,
    token_address TEXT NOT NULL,
    balance TEXT NOT NULL,
    -- "ERC721" or "ERC1155" (null for "ERC20")
    FOREIGN KEY (token_id) REFERENCES token_id(token_id),
    -- make token_address foreign key
    FOREIGN KEY (token_address) REFERENCES erc_contracts(token_address),
);

CREATE TABLE erc20_transfers (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    token_address TEXT NOT NULL,
    from_address TEXT NOT NULL,
    to_address TEXT NOT NULL,
    amount TEXT NOT NULL
);

CREATE TABLE erc721_transfers (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    token_address TEXT NOT NULL,
    from_address TEXT NOT NULL,
    to_address TEXT NOT NULL,
    token_id TEXT NOT NULL
);

-- these are metadata of the contracts which we would need to fetch from RPC separately
-- not part of events engine

CREATE TABLE erc20_contracts (
    token_address TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL, 
    symbol TEXT NOT NULL,
    decimals INTEGER NOT NULL,
    total_supply TEXT NOT NULL
);

CREATE TABLE erc721_contracts (
    token_address TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    symbol TEXT NOT NULL,
    total_supply TEXT NOT NULL
);

-- -- 
-- CREATE TABLE contracts (
--     id TEXT NOT NULL PRIMARY KEY,
--     contract_address TEXT NOT NULL,
--     contract_type TEXT NOT NULL,
--     head TEXT NOT NULL,
-- )

-- CREATE TABLE balances (
--     id TEXT NOT NULL PRIMARY KEY,
--     balance TEXT NOT NULL,
--     account_address TEXT NOT NULL,
--     contract_address TEXT NOT NULL,
--     token_id TEXT,
--     FOREIGN KEY (token_id) REFERENCES tokens(id),
-- )

-- CREATE INDEX balances_account_address ON balances (account_address);

-- CREATE TABLE tokens (
--     id TEXT NOT NULL PRIMARY KEY,
--     uri TEXT NOT NULL,
--     contract_address TEXT NOT NULL,
--     FOREIGN KEY (contract_address) REFERENCES contracts(contract_address),
-- )