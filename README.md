# Dōjō

Dojo is a full stack toolchain for developing onchain games in Cairo. Dojo leverages the afforadances provided by the Cairo language to offer an best-in-class developer experience for easily integration blockchain properties into their games.

- Simple composition through the Entity Component System pattern
- Concise implementations leveraging language plugins and macros
- Expressive query system with efficiently compiled strategies
- Typed interface generation for client libraries

The toolchain includes the following:
- `dojo-ecs`: An concise and efficient implementation of the Entity Component System pattern.
- `dojo-migrate`: Deploy, migrate, and manage the entities, components, and systems in the world.
- `dojo-bind`: Generate bindings for various languages / frameworks (typescript, phaser / rust, bevy).

## Development
### Prerequisites
- Install [Rust](https://www.rust-lang.org/tools/install)
- Setup Rust:
```
rustup override set stable && rustup update && cargo test
```

## Overview

### Entity Component System

Dojo implements the ECS pattern which is subsequently compiled to Starknet contracts for deployment. The syntax and semantics are heavily inspired by [Bevy](https://bevyengine.org/).

#### Worlds

A `world` is the top-level concept in an onchain game, serving as a centralized registry, namespace, and event bus for all entities, components, systems, and resources.

The worlds interface is as follows:

```rust
trait World {
    // Register a component or system. The returned
    // hash is used to uniquely identify the component or
    // system in the world. All components and systems
    // within a world are deteriministically addressed
    // relative to the world.
    // @TODO: Figure out how to propagate calldata with Cairo 1.0.
    fn register(id: felt, class_hash: felt) -> felt;

    // Called when a component in the world updates the value
    // for an entity. When called for the first time for an 
    // entity, the entity:component mapping is registered.
    // Additionally, a `ComponentValueSet` event is emitted.
    fn on_component_set(entity_id: felt, data: Array::<felt>);

    // Lookup entities that have a component by id.
    fn lookup(id: felt) -> Array::<felt>;
}
```

#### Components

Components in `dojo-ecs` are plain structs, for example, the following implements a `Position` component which exposes a `is_zero` method.

```rust
mod position {
    #[derive(Component)]
    struct Position { x: felt, y: felt }

    trait IPosition {
        fn is_zero(self: Position) -> bool;
    }

    // @NOTE: Seems plain impl isn't supported yet, we need to have a trait
    impl Position of IPosition {
        #[view]
        fn is_zero(self: Position) -> bool {
            match self.x - self.y {
                0 => bool::True(()),
                _ => bool::False(()),
            }
        }

        #[view]
        fn is_equal(self: Position, b: Position) -> bool {
            self.x == b.x & self.y == b.y
        }
    }
}
```

Components are then expanded to Starknet contract:

```rust
#[contract]
mod Position {
    #[derive(Component)]
    struct Position {
        x: felt,
        y: felt
    }

    struct Storage {
        world_address: felt,
        state: Map::<felt, Position>,
    }

    // Initialize PositionComponent.
    #[external]
    fn initialize(world_addr: felt) {
        let world = world_address::read();
        assert(world == 0, 'PositionComponent: Already initialized.');
        world_address::write(world_addr);
    }

    // Set the state of an entity.
    #[external]
    fn set(entity_id: felt, value: Position) {
        state::write(entity_id, value);
    }

    // Get the state of an entity.
    #[view]
    fn get(entity_id: felt) -> Position {
        return state::read(entity_id);
    }


    #[view]
    fn is_zero(entity_id: felt) -> bool {
        let self = state::read(entity_id);
        match self.x - self.y {
            0 => bool::True(()),
            _ => bool::False(()),
        }
    }

    #[view]
    fn is_equal(entity_id: felt, b: Position) -> bool {
        let self = state::read(entity_id);
        self.x == b.x & self.y == b.y
    }
}
```

In the expanded form, entrypoints take `entity_id` as the first parameter.

#### Systems

A system is a free function that takes as input a set of entities to operate on. Systems define a `Query` which describes a set of Components to query a worlds entities by. At compile time, the `Query` is compiled, leveraging [deterministic addresssing](#Addressing) to inline efficient entity lookups.

```rust
fn move(query: Query<(Position, Health)>) {
    // @NOTE: Loops are not available in Cairo 1.0 yet.
    for (position, health) in query {
        let is_zero = position.is_zero();
    }
    return ();
}
```

Expansion:

```rust
#[contract]
mod MoveSystem {
    struct Storage {
        world_address: felt,
    }

    #[external]
    fn initialize(world_addr: felt) {
        let world = world_address::read();
        assert(world == 0, 'MoveSystem: Already initialized.');
        world_address::write(world_addr);
    }

    #[external]
    fn execute() {
        let world = world_address::read();
        assert(world != 0, 'MoveSystem: Not initialized.');

        let position_id = pedersen("PositionComponent");
        // We can compute the component addresses statically
        // during compilation.
        let position_address = compute_address(position_id);
        let position_entities = IWorld.lookup(world, position_id);

        let health_id = pedersen("HealthComponent");
        let health_address = compute_address(health_id);
        let health_entities = IWorld.lookup(world, health_id);

        let entities = intersect(position_entities, health_entities);

        for entity_id in entities {
            let is_zero = IPosition.is_zero(position_address, entity_id);
        }
    }
}
```

#### Entities

An entity is addressed by a `felt`. An entity represents a collection of component state. A component can set state for an arbitrary entity, registering itself with the world as a side effect.


#### Addressing

Everything inside a Dojo World is deterministically addressed relative to the world, from the address of a system to the storage slot of an entities component value. This is accomplished by enforcing module name uniqueness, i.e. `PositionComponent` and `MoveSystem`, wrapping all components and systems using the proxy pattern, and standardizing the storage layout of component modules.

This property allows for:
1) Statically planning deployment and migration strategies for updates to the world
2) Trustlessly recreating world state on clients using a [light client with storage proofs](https://github.com/keep-starknet-strange/beerus)
3) Optimistically updating client state using [client computed state transitions](https://github.com/starkware-libs/blockifier)
4) Efficiently querying a subset of the world state without replaying event history

```rust
use starknet::{deploy, pedersen};

impl World {
    struct Storage {
        registry: Map::<felt, felt>,
    }

    fn register(class_hash: felt) -> felt {
        let module_id = pedersen("PositionComponent");
        let address = deploy(
            class_hash=proxy_class_hash,
            contract_address_salt=module_id,
            constructor_calldata_size=0,
            constructor_calldata=[],
            deploy_from_zero=FALSE,
        );
        IProxy.set_implementation(class_hash);
        IPositionComponent.initialize(address, ...);
        registry.write(module_id, address);
    }
}
```

#### Events

Events are emitted anytime a components state is updated a `ComponentValueSet` event is emitted from the world, enabling clients to easily track changes to world state.

### Migrate

Given addresses of every component / system in the world is deterministically addressable, the `dojo-migrate` cli takes a world address as entrypoint and diffs the onchain state with the compiled state, generating a deploying + migration plan for declaring and registering new components and / or updating existing components.

### Bind

Bind is a cli for generating typed interfaces for integration with various client libraries / languages.
