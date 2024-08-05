CREATE TABLE Erc20Balance (
    address TEXT NOT NULL,
    token_address TEXT NOT NULL,
    balance TEXT NOT NULL,
    PRIMARY KEY (address, token_address)
);

CREATE TABLE Erc721Balance (
    address TEXT NOT NULL,
    token_address TEXT NOT NULL,
    token_id TEXT NOT NULL,
    token_uri TEXT NOT NULL,
    PRIMARY KEY (address, token_address, token_id)
)

-- these are metadata of the contracts which we would need to fetch from RPC separately
-- not part of events engine

-- Do we need this?

CREATE TABLE Erc20Contract (
    address TEXT NOT NULL PRIMARY KEY,
    decimals INTEGER NOT NULL,
    name TEXT NOT NULL, 
    symbol TEXT NOT NULL,
    total_supply TEXT NOT NULL,
)

CREATE TABLE Erc721Contract (
    address TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    symbol TEXT NOT NULL,
    total_supply TEXT NOT NULL,
)