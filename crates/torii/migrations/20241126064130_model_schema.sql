-- Adds a new schema column to the models table.
-- The schema is the JSON serialized Ty of the model.
ALTER TABLE models ADD COLUMN schema BLOB NOT NULL;
