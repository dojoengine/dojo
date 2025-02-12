-- The token id that the token refers too. This is nullable as erc20 are also in the tokens table.
ALTER TABLE tokens
ADD COLUMN token_id TEXT NULL;

-- Add an index on the token_id column
CREATE INDEX idx_token_id ON tokens (token_id);