-- Ensures entities can be stored as historical.
-- The historicallity is achieved by storing a counter for each pair <entity_id, model_selector>.
CREATE TABLE entities_historical (
    -- No primary key, since we are storing 1-M relationship
    -- to retrieve all historical entities for a given entity_id.
    id TEXT NOT NULL,
    keys TEXT,
    event_id TEXT NOT NULL,
    -- The serialized data of the entity, which contains the Ty.
    data TEXT NOT NULL,
    -- The model id of the serialized data.
    model_id TEXT NOT NULL,
    executed_at DATETIME NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- The counter added on <entity_model> is merely used to avoid querying a
-- potentially big table to get the latest counter for a given <entity_id, model_selector>.
ALTER TABLE entity_model ADD COLUMN historical_counter BIGINT DEFAULT 0;
