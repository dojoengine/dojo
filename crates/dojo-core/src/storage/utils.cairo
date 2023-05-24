use array::{ArrayTrait, SpanTrait};
use dict::Felt252DictTrait;
use option::OptionTrait;
use traits::{Into, TryInto};

use dojo_core::integer::u250;

// big enough number used to construct a compound key in `find_matching`
const OFFSET: felt252 = 0x10000000000000000000000000000000000; 

// finds only those entities that have same IDs across all provided entities and 
// returns these entities, obeying the order of IDs of the first ID array
//
// the function takes two aruments:
// * `ids` is a list of lists of entity IDs; each inner list is an ID of an entity
//    at the same index in the corresponding `entities` list
// * `entities` is a list of lists of deserialized entities; each list of entities
//   is of the same entity type in the order of IDs from `ids
//
// to illustrate, consider we have two entity types (components), Place and Owner
// `ids` are [[4, 2, 3], [3, 4, 5]]
// `entities` are [[P4, P2, P3], [O3, O4, O5]]
// where P4 is a deserialized (i.e. a Span<felt252>) Place entity with ID 4,
// O3 is a deserialized Owner entity with ID 4 and so on..
// 
// the function would return [[P4, P3], [O4, O3]] because IDs 3 and 4 are found
// for all entities and the function respects the ID order from the first ID array,
// hence 4 and 3 in this case
fn find_matching(
    mut ids: Span<Span<u250>>,
    mut entities: Span<Span<Span<felt252>>> 
) -> Span<Span<Span<felt252>>> {
    assert(ids.len() == entities.len(), 'lengths dont match');

    let entity_types_count = entities.len();
    if entity_types_count == 1 {
        return entities;
    }

    // keeps track of how many times has an ID been encountered
    let mut ids_match: Felt252Dict<u8> = Felt252DictTrait::new();

    // keeps track of indexes where a particular entity with ID is in
    // each entity array; to do so, we're using a compound key or 2 parts
    // first part is the entity *type* (calculated as OFFSET * entity_type_counter)
    // second part is the entity ID itself
    let mut id_to_idx: Felt252Dict<usize> = Felt252DictTrait::new();

    // how many ID arrays have we looped over so far; ultimately
    // this number is the same as ids.len() and we use only those
    // IDs from ids_match where the value is the same as match_count
    
    // we want to keep the ordering from the first entity IDs
    let mut ids1: Span<u250> = *(ids.pop_front().unwrap());

    // counts how many ID arrays and hence entity types we've looped over
    // starts at 1 because we skip the first element to keep ordering (see above)
    let mut entity_type_counter: u8 = 1;

    loop {
        // loop through the rest of the IDs for entity types 2..N
        match ids.pop_front() {
            Option::Some(entity_ids) => {
                let mut index: usize = 0;
                let mut entity_ids = *entity_ids;

                loop {
                    // loop through each ID of an entity type
                    match entity_ids.pop_front() {
                        Option::Some(id) => {
                            // keep track how many times we've encountered a particular ID
                            let c = ids_match[*id.inner];                            
                            ids_match.insert(*id.inner, c + 1);
                            // keep track of the index of the particular entity in an
                            // entity type array, i.e. at which index is the entity
                            // with `id` at, using the compound key
                            id_to_idx.insert(OFFSET * entity_type_counter.into() + *id.inner, index);
                            index += 1;
                        },
                        Option::None(_) => {
                            break ();
                        }
                    };
                };
                
                entity_type_counter += 1;
            },
            Option::None(_) => {
                break ();
            }
        };
    };

    let first_entities: Span<Span<felt252>> = *entities[0];
    let mut first_entities_idx = 0;

    // an array into which we append those entities who's IDs are found across
    // every ID array; the entities are appended sequentially, e.g.
    // [entity1_id1, entity2_id1, entity3_id1, entity1_id2, entity2_id2, entity3_id2]
    // perserving the ID order from the first ID array
    let mut entities_with_matching_ids: Array<Span<felt252>> = ArrayTrait::new();

    let found_in_all: u8 = entity_type_counter - 1;

    loop {
        match ids1.pop_front() {
            Option::Some(id) => {
                let id = *id.inner;
                if ids_match[id] == found_in_all {
                    // id was found in every entity_ids array

                    // append the matching entity to the array
                    entities_with_matching_ids.append(*first_entities[first_entities_idx]);

                    // now append all the other matching entities there too
                    let mut entity_types_idx = 1;
                    loop {
                        if entity_types_idx == entity_types_count {
                            break ();
                        }
                        let idx_for_matching_id = id_to_idx[OFFSET * entity_types_idx.into() + id];
                        let same_type_entities = entities[entity_types_idx];
                        entities_with_matching_ids.append(*same_type_entities[idx_for_matching_id]);

                        entity_types_idx += 1;
                    }
                }
                first_entities_idx += 1;
            },
            Option::None(_) => {
                break ();
            }
        };
    };

    ids_match.squash();
    id_to_idx.squash();

    let mut entities_with_matching_ids = entities_with_matching_ids.span();
    // calculate how many common IDs across all entities we found
    // guaranteed to be a round number
    let matches = entities_with_matching_ids.len() / entity_types_count; 
    let mut result: Array<Span<Span<felt252>>> = ArrayTrait::new();
    let mut i = 0;

    // finally, reorder the entities from the temporary array
    // into the resulting one in a way where they are grouped together by
    // entity type
    loop {
        if i == entity_types_count {
            break ();
        }

        let mut j = 0;
        let mut same_entities: Array<Span<felt252>> = ArrayTrait::new();
        loop {
            if j == matches {
                break ();
            }
            same_entities.append(*entities_with_matching_ids[j * entity_types_count + i]);
            j += 1;
        };

        result.append(same_entities.span());
        i += 1;
    };

    result.span()
}
