// @generated automatically by Diesel CLI.

diesel::table! {
    components (id) {
        id -> Nullable<Text>,
        name -> Nullable<Text>,
        properties -> Nullable<Text>,
        address -> Text,
        class_hash -> Text,
        transaction_hash -> Text,
    }
}

diesel::table! {
    entities (id) {
        id -> Nullable<Text>,
        name -> Nullable<Text>,
        transaction_hash -> Text,
    }
}

diesel::table! {
    entity_state_updates (id) {
        id -> Nullable<Integer>,
        entity_id -> Text,
        component_id -> Text,
        transaction_hash -> Text,
        data -> Nullable<Text>,
    }
}

diesel::table! {
    entity_states (id) {
        id -> Nullable<Integer>,
        entity_id -> Text,
        component_id -> Text,
        data -> Nullable<Text>,
    }
}

diesel::table! {
    system_calls (id) {
        id -> Nullable<Integer>,
        data -> Nullable<Text>,
        transaction_hash -> Text,
        system_id -> Text,
    }
}

diesel::table! {
    systems (id) {
        id -> Nullable<Text>,
        name -> Nullable<Text>,
        address -> Text,
        class_hash -> Text,
        transaction_hash -> Text,
    }
}

diesel::joinable!(entity_state_updates -> components (component_id));
diesel::joinable!(entity_state_updates -> entities (entity_id));
diesel::joinable!(entity_states -> components (component_id));
diesel::joinable!(entity_states -> entities (entity_id));

diesel::allow_tables_to_appear_in_same_query!(
    components,
    entities,
    entity_state_updates,
    entity_states,
    system_calls,
    systems,
);
