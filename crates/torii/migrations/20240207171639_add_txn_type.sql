ALTER TABLE
    transactions
ADD
    COLUMN transaction_type TEXT NOT NULL DEFAULT 'INVOKE';