-- Models have now a contract address.
ALTER TABLE models
ADD COLUMN contract_address TEXT DEFAULT '0' NOT NULL;
