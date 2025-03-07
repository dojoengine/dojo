-- We parse the calls from the transaction calldata
-- and outside calls from for eg. paymaster transactions
ALTER TABLE transactions 
ADD COLUMN outside_calls TEXT
ADD COLUMN calls TEXT