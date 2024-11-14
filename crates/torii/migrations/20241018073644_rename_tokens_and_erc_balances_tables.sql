-- Add migration script here
ALTER TABLE balances RENAME TO token_balances;
ALTER TABLE erc_transfers RENAME TO token_transfers;
