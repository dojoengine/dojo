use diesel::prelude::*;

#[derive(Clone, Debug, Identifiable, Queryable, Selectable, PartialEq)]
#[diesel(primary_key(component_id, entity_id))]
#[diesel(belongs_to(Component))]
#[diesel(belongs_to(Entity))]
#[diesel(table_name = entity_states)]
pub struct EntityState {
    pub component_id: String,
    pub entity_id: String,
    pub data: String,
}

#[derive(Clone, Debug, Identifiable, Queryable, Selectable, PartialEq)]
#[diesel(primary_key(id))]
#[diesel(belongs_to(Component))]
#[diesel(belongs_to(Entity))]
#[diesel(table_name = entity_state_updates)]
pub struct EntityStateUpdate {
    pub id: i32,
    pub component_id: String,
    pub entity_id: String,
    pub data: String,
    pub transaction_hash: String,
}

#[derive(Clone, Debug, Identifiable, Queryable, Selectable, PartialEq)]
#[diesel(primary_key(id))]
#[diesel(table_name = components)]
pub struct Component {
    pub id: String,
    pub name: String,
    pub properties: String,
    pub address: String,
    pub class_hash: String,
    pub transaction_hash: String,
}

#[derive(Clone, Debug, Identifiable, Queryable, Selectable, PartialEq)]
#[diesel(primary_key(id))]
#[diesel(table_name = systems)]
pub struct System {
    pub id: String,
    pub name: String,
    pub address: String,
    pub class_hash: String,
    pub transaction_hash: String,
}

#[derive(Clone, Debug, Identifiable, Queryable, Selectable, PartialEq)]
#[diesel(primary_key(id))]
#[diesel(belongs_to(System))]
#[diesel(table_name = system_calls)]
pub struct SystemCall {
    pub id: i32,
    pub data: String,
    pub system_id: String,
    pub transaction_hash: String,
}

#[derive(Clone, Debug, Identifiable, Queryable, Selectable, PartialEq)]
#[diesel(primary_key(id))]
#[diesel(table_name = entities)]
pub struct Entity {
    pub id: String,
    pub name: String,
    pub transaction_hash: String,
}