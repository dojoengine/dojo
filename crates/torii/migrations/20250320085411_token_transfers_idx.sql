-- Add indexes to the token_transfers table on contract_address, from_address, to_address, event_id
CREATE INDEX idx_token_transfers_contract_address ON token_transfers (contract_address);
CREATE INDEX idx_token_transfers_from_address ON token_transfers (from_address);
CREATE INDEX idx_token_transfers_to_address ON token_transfers (to_address);
CREATE INDEX idx_token_transfers_event_id ON token_transfers (event_id);
