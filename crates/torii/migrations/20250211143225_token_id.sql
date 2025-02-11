-- The token id that the token refers too. This is nullable as erc20 are also in the tokens table.
ALTER TABLE tokens
ADD COLUMN token_id TEXT NULL;

-- Add an index on the token_id column
CREATE INDEX idx_token_id ON tokens (token_id);

-- Add a unique constraint on the contract_address and token_id columns
ALTER TABLE tokens
ADD CONSTRAINT unique_contract_address_token_id UNIQUE (contract_address, token_id);
