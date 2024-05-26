-- Add migration script here
-- CREATE TABLE model_members(
--     id TEXT NOT NULL,
--     model_idx INTEGER NOT NULL,
--     member_idx INTEGER NOT NULL,
--     model_id TEXT NOT NULL,
--     name TEXT NOT NULL,
--     type TEXT NOT NULL,
--     type_enum TEXT DEFAULT 'Primitive' CHECK(
--         type_enum IN ('Primitive', 'Struct', 'Enum', 'Tuple')
--     ) NOT NULL,
--     enum_options TEXT NULL,  -- TEMP: Remove once enum support is properly added
--     key BOOLEAN NOT NULL,
--     -- TEMP: Remove CURRENT_TIMESTAMP
--     executed_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
--     created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
--     PRIMARY KEY (id, member_idx) FOREIGN KEY (model_id) REFERENCES models(id)
-- );
-- modify this table to 'Array' and 'ByteArray' to type_enum

ALTER TABLE model_members
    RENAME TO model_members_old;

CREATE TABLE model_members(
    id TEXT NOT NULL,
    model_idx INTEGER NOT NULL,
    member_idx INTEGER NOT NULL,
    model_id TEXT NOT NULL,
    name TEXT NOT NULL,
    type TEXT NOT NULL,
    type_enum TEXT DEFAULT 'Primitive' CHECK(
        type_enum IN ('Primitive', 'Struct', 'Enum', 'Tuple', 'Array', 'ByteArray')
    ) NOT NULL,
    enum_options TEXT NULL,  -- TEMP: Remove once enum support is properly added
    key BOOLEAN NOT NULL,
    -- TEMP: Remove CURRENT_TIMESTAMP
    executed_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id, member_idx) FOREIGN KEY (model_id) REFERENCES models(id)
);

INSERT INTO model_members (id, model_idx, member_idx, model_id, name, type, type_enum, key, executed_at, created_at)
SELECT id, model_idx, member_idx, model_id, name, type, type_enum, key, executed_at, created_at
FROM model_members_old;

-- Disable foreign keys constraint so we can delete model_members_old
PRAGMA foreign_keys = OFF;

DROP TABLE model_members_old;

-- Renable foreign keys
PRAGMA foreign_keys = ON;

CREATE INDEX idx_model_members_model_id ON model_members (model_id);
