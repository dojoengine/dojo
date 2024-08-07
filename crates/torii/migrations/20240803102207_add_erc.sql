CREATE TABLE erc20_balances (
    address TEXT NOT NULL,
    token_address TEXT NOT NULL,
    balance TEXT NOT NULL,
    PRIMARY KEY (address, token_address)
);

CREATE TABLE erc721_balances (
    address TEXT NOT NULL,
    token_address TEXT NOT NULL,
    token_id TEXT NOT NULL,
    PRIMARY KEY (address, token_address, token_id)
);

CREATE TABLE erc20_transfers (
    address TEXT NOT NULL,
    token_address TEXT NOT NULL,
    from_address TEXT NOT NULL,
    to_address TEXT NOT NULL,
    amount TEXT NOT NULL,
    PRIMARY KEY (address, token_address)
);

CREATE TABLE erc721_transfers (
    address TEXT NOT NULL,
    token_address TEXT NOT NULL,
    from_address TEXT NOT NULL,
    to_address TEXT NOT NULL,
    token_id TEXT NOT NULL,
    PRIMARY KEY (address, token_address, token_id)
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