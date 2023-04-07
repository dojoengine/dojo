<picture>
  <source media="(prefers-color-scheme: dark)" srcset=".github/mark-dark.svg">
  <img alt="Dojo logo" align="right" width="120" src=".github/mark-light.svg">
</picture>

## Dojo

![Github Actions][gha-badge] [![Telegram Chat][tg-badge]][tg-url]

[gha-badge]: https://img.shields.io/github/actions/workflow/status/dojoengine/dojo/ci.yml?branch=main
[tg-badge]: https://img.shields.io/endpoint?color=neon&logo=telegram&label=chat&style=flat-square&url=https%3A%2F%2Ftg.sumanjay.workers.dev%2Fdojoengine
[tg-url]: https://t.me/dojoengine

**Dojo is a toolchain for building Autonomous Worlds in Cairo.**

Dojo provides:

- Scalable composition through the Entity Component System pattern.
- Permissionless expansion of autonomous worlds through the introduction of components and systems.
- Concise macros for interacting with world state and functions.
- Typed interface generation for client libraries (Coming soon)

## Overview

### Entity Component System

Dojo implements the ECS pattern to enable modular and extensible autonomous worlds. Worlds can be permissionlessly expanded over time through the incorporation of components and systems.

#### World

The `world` is the top-level concept in an onchain game, serving as a centralized registry, namespace, and event bus for all entities, components, systems, and resources.

#### Components

Components form the schema of the world, holding state for systems to operate on. Components struct, for example, the following implements a `Position` component which exposes a `is_zero` and `is_equal` method. The Dojo toolchain compiles components to contracts which can be declared and installed into a world.

##### Example

```rust
#[component]
struct Position {
    x: u32,
    y: u32
}

trait PositionTrait {
    fn is_equal(self: Position, b: Position) -> bool;
}

impl PositionImpl of PositionTrait {
    fn is_equal(self: Position, b: Position) -> bool {
        self.x == b.x & self.y == b.y
    }
}
```

#### Systems

Systems are functions operating on the world state. They receive some input from the user, retreive state from the world, compute a state transition and apply it. A system has a single entrypoint, the `execute` function. Systems can leverage `commands` to easily interact with the world.


##### Commands

```rust
// Retrieve a unique id from the world, useful for create a new entity.
fn commands::uuid() -> felt252;

// Update an existing entity with the provided components.
fn commands::set_entity(storage_key: StorageKey, components: T);

// Retreive a components for an entity.
fn commands::<T>::entity(storage_key: StorageKey) -> T;

// Retreive all entity ids that match the component selector criteria.
fn commands::<T>::entities() -> Array<felt252>;
```

##### Example

```rust
#[system]
mod SpawnSystem {
    fn execute(name: String) {
        let player_id = commands::create((
            Health::new(100_u8),
            Name::new(name)
        ));
        return ();
    }
}

#[system]
mod MoveSystem {
    fn execute(player_id: usize) {
        let player = commands<(Health, Name)>::entity(player_id);
        let positions = commands<(Position, Health)>::entities();

        // @NOTE: Loops are not available in Cairo 1.0 yet.
        for (position, health) in positions {
            let is_zero = position.is_zero();
        }
        return ();
    }
}
```

#### Entities

An entity is addressed by a `felt252`. An entity represents a collection of component state.

## Development

### Dev Container

It is recommended to use the dev container when building on Dojo as it contains everything needed to begin developing.

Make sure you update your Docker to the latest stable version, sometimes the Dev containers do not play nicely with old Docker versions.

#### Restart VSCode for this to take effect

##### Open and build container

Command pallete: `ctrl + shift + p`

Then: `Remote-Containers: Rebuild Container Without Cache`

##### Setup the language server 

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