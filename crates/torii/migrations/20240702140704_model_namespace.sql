-- Models have now a namespace.
ALTER TABLE models
ADD COLUMN namespace TEXT NOT NULL;
