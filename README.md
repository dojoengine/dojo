# Dōjō

Dojo is a full stack toolchain for developing onchain games in Cairo. Dojo leverages the affordances provided by Cairo language plugins to offer a best-in-class developer experience for easily integrating blockchain properties into games.

- Simple composition through the Entity Component System pattern
- Concise implementations leveraging language plugins and macros
- Expressive query system with efficiently compiled strategies
- Typed interface generation for client libraries

The toolchain includes the following:
- `dojo-ecs`: A concise and efficient implementation of the Entity Component System pattern.
- `dojo-migrate`: Deploy, migrate, and manage the entities, components, and systems in the world.
- `dojo-bind`: Generate bindings for various languages / frameworks (typescript, phaser / rust, bevy).

## Development

### Setup Submodules

```
git submodule update --init --recursive
```

## Development container

It is recommended to use the dev container when building on DoJo as it contains everything needed to begin developing.

Make sure you update your Docker to the latest stable version, sometimes the Dev containers do not play nicely with old Docker versions.

Due to ARM and x86 issues with Docker you need to set the Variant for the container in your bashprofile.


### Before building
Before building the container you need to add in the `VARIANT` so the correct image is built according to your machine.
```
sudo nano ~./bash_profile
```

Then beneath the other statements either add:

```bash
# for M1/M2 Apple chips
export VARIANT="bullseye"

# or

# for all others
export VARIANT="buster"

# Do not add both!

# Restart VSCode for this to take effect
```

### Open and build container

Command pallete: `ctrl + shift + p`

Then: `Remote-Containers: Rebuild Container Without Cache`

### Setup the language server 

```
cd cairo/vscode-cairo

npm install --global @vscode/vsce
npm install
vsce package
code --install-extension cairo1*.vsix

cd /workspaces/dojo

cargo build --bin dojo-language-server --release
```

### Development without container

- Install [Rust](https://www.rust-lang.org/tools/install)
- Setup Rust:
```
rustup override set stable && rustup update && cargo test
```
Then install the language like described above.

---

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
    // within a world are deterministically addressed
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

Components in `dojo-ecs` are modules with a single struct describing its state, for example, the following implements a `Position` component which exposes a `is_zero` and `is_equal` method.

```rust
#[component]
mod PositionComponent {
    struct Position {
        x: felt,
        y: felt
    }

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
```

#### Systems

A system is a pure function that takes as input a set of entities to operate on. Systems define a `Query` which describes a set of criteria to query entities with.

```rust
#[system]
mod MoveSystem {
    fn execute(query: Query<(Position, Health)>) {
        // @NOTE: Loops are not available in Cairo 1.0 yet.
        for (position, health) in query {
            let is_zero = position.is_zero();
        }
        return ();
    }
}
```

#### Entities

An entity is addressed by a `felt`. An entity represents a collection of component state. A component can set state for an arbitrary entity, registering itself with the world as a side effect.


#### Addressing

Everything inside a Dojo World is deterministically addressed relative to the world, from the address of a system to the storage slot of an entity's component value. This is accomplished by enforcing module name uniqueness, i.e. `PositionComponent` and `MoveSystem`, wrapping all components and systems using the proxy pattern, and standardizing the storage layout of component modules.

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
