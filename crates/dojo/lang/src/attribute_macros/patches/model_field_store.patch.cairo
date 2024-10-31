    fn get_$field_name$(self: @S, key: $model_type$KeyType) -> $field_type$ {
        $model_type$Store::get_member(self, key, $field_selector$)
    }

    fn get_$field_name$_from_id(self: @S, entity_id: felt252) -> $field_type$ {
        $model_type$ModelValueStore::get_member_from_id(self, entity_id, $field_selector$)
    }

    fn update_$field_name$(ref self: S, key: $model_type$KeyType, value: $field_type$) {
        $model_type$Store::update_member(ref self, key, $field_selector$, value);
    }

    fn update_$field_name$_from_id(ref self: S, entity_id: felt252, value: $field_type$) {
        $model_type$ModelValueStore::update_member_from_id(ref self, entity_id, $field_selector$, value);
    }
