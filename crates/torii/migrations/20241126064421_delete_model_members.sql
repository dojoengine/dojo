-- Deletes the model_members table. Which is no longer needed since we store the schema in the models table.
PRAGMA foreign_keys = OFF;
DROP TABLE model_members;
PRAGMA foreign_keys = ON;
