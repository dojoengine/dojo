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

-- -- one row represents a token contract on chain
-- CREATE TABLE erc_contracts (
--     token_address TEXT NOT NULL PRIMARY KEY,
--     -- "ERC20" or "ERC721" or "ERC1155"
--     token_type TEXT NOT NULL,
--     --! for ERC1155: both name and symbol are offchain (so would be null)
--     name TEXT,
--     symbol TEXT,
--     --! Null for ERC721 and its part of metadata in ERC1155 (so needs to be fetched offchain)
--     decimals TEXT,
--     --! total_supply would in erc1155 would need to be a map of token_id to balance
--     total_supply TEXT
-- );

-- CREATE TABLE erc_balances (
--     -- for ERC20, this would be (account_address:token_address:0x0)
--     -- for ERC721 and ERC1155, this would be (account_address:token_address:token_id)
--     id TEXT NOT NULL PRIMARY KEY,
--     account_address TEXT NOT NULL,
--     token_address TEXT NOT NULL,
--     -- "ERC721" or "ERC1155" (null for "ERC20")
--     token_id TEXT NOT NULL,
--     balance TEXT NOT NULL,
--     -- make token_address foreign key
--     FOREIGN KEY (token_address) REFERENCES erc_contracts(token_address),
-- );

CREATE TABLE erc20_transfers (
    account_address TEXT NOT NULL,
    token_address TEXT NOT NULL,
    from_address TEXT NOT NULL,
    to_address TEXT NOT NULL,
    amount TEXT NOT NULL,
    PRIMARY KEY (account_address, token_address)
);

CREATE TABLE erc721_transfers (
    account_address TEXT NOT NULL,
    token_address TEXT NOT NULL,
    from_address TEXT NOT NULL,
    to_address TEXT NOT NULL,
    token_id TEXT NOT NULL,
    PRIMARY KEY (account_address, token_address, token_id)
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