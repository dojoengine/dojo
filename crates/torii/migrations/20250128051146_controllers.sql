-- Cartridge controllers
CREATE TABLE controllers (
    id TEXT PRIMARY KEY NOT NULL,  -- Username as primary key
    username TEXT NOT NULL,        -- Username
    address TEXT NOT NULL,         -- Wallet address
    deployed_at TIMESTAMP NOT NULL -- Block timestamp of deployment
);

CREATE INDEX idx_controllers_username ON controllers (username);
CREATE INDEX idx_controllers_address ON controllers (address);
