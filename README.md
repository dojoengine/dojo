<picture>
  <source media="(prefers-color-scheme: dark)" srcset=".github/mark-dark.svg">
  <img alt="Dojo logo" align="right" width="120" src=".github/mark-light.svg">
</picture>

## Dojo
<a href="https://twitter.com/dojostarknet">
<img src="https://img.shields.io/twitter/follow/dojostarknet?style=social"/>
</a>

[![discord](https://img.shields.io/badge/join-dojo-green?logo=discord&logoColor=white)](https://discord.gg/PwDa2mKhR4)
![Github Actions][gha-badge] [![Telegram Chat][tg-badge]][tg-url]


[gha-badge]: https://img.shields.io/github/actions/workflow/status/dojoengine/dojo/ci.yml?branch=main
[tg-badge]: https://img.shields.io/endpoint?color=neon&logo=telegram&label=chat&style=flat-square&url=https%3A%2F%2Ftg.sumanjay.workers.dev%2Fdojoengine

[tg-url]: https://t.me/dojoengine

**Dojo is a community driven open-source, Provable Game Engine, providing a comprehensive toolkit for building verifiable games and autonomous worlds.**

Dojo is still in its early stages of development, yet the dedicated contributors are propelling its progress at an impressive pace. The overarching aspiration for Dojo is to empower game developers to kick-start their projects, aiming to reduce the initial setup time from days to mere hours. Join the movement!

## 🔑 Key Features
- Cairo 1.0 Entity Component System (ECS)
- Sozu migration planner
- [Torii](/crates/torii/README.md) networking & indexing stack
- [Katana](/crates/katana/README.md) RPC development network
- Typed SDKs


## 🚀 Quick Start

See the [installation guide](https://book.dojoengine.org/getting-started/installation.html) in the book.

## 🗒️ Documentation
You can find more detailed documentation in the Dojo Book [here](https://book.dojoengine.org/).

## ❓ Support
If you encounter issues or have questions, you can [submit an issue on GitHub](https://github.com/dojoengine/dojo/issues). You can also join our [Discord](https://discord.gg/PwDa2mKhR4) for discussion and help.

## ✏️ Contributing
We welcome contributions of all kinds from anyone. See our [Contribution Guide](/CONTRIBUTING.md) for more information on how to get involved.

## ✏️ Enviroment
See our [Enviroment setup](https://book.dojoengine.org/development/enviroment.html) for more information.

## ⛩️ Built with Dojo 
- [Roll Your Own](https://github.com/cartridge-gg/rollyourown)
- [Realms Autonomous World](https://github.com/BibliothecaDAO/eternum)

---

## Dojo Core Overview

- [ECS](#entity-component-system)
- - [World](#world)
- - [Components](#world)
- - [Systems](#world)

### Entity Component System

Dojo implements the ECS pattern to enable modular and extensible Autonomous Worlds. Worlds can be permissionlessly expanded over time through the incorporation of components and systems.

#### World

The `world` is the top-level concept in a Autonomous World, serving as a centralized registry, namespace, and event bus for all entities, components, systems, and resources.

#### Components

Components form the schema of the world, holding state for systems to operate on. Components struct, for example, the following implements a `Position` component which exposes a `is_zero` and `is_equal` method. The Dojo toolchain compiles components to contracts which can be declared and installed into a world.

##### Components Example

```rust
#[derive(Component, Copy, Drop, Serde)]
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

Systems are functions operating on the world state. They receive some input from the user, retrieve state from the world, compute a state transition and apply it. A system has a single entrypoint, the `execute` function. Systems can leverage `commands` to easily interact with the world.

##### Commands

```rust
// Retrieve a unique id from the world, useful for create a new entity.
fn commands::uuid() -> felt252;

// Update an existing entity with the provided components.
fn commands::set_entity(query: Query, components: T);

// Retrieve a components for an entity.
fn commands::<T>::entity(query: Query) -> T;

// Retrieve all entity ids that match the component selector criteria.
fn commands::<T>::entities() -> Array<felt252>;
```

##### System Example

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

An entity is addressed by a `felt250`. An entity represents a collection of component state.


## Contributors ✨

Thanks goes to these wonderful people
([emoji key](https://allcontributors.org/docs/en/emoji-key)):

<!-- ALL-CONTRIBUTORS-LIST:START - Do not remove or modify this section -->
<!-- prettier-ignore-start -->
<!-- markdownlint-disable -->

<!-- markdownlint-restore -->
<!-- prettier-ignore-end -->

<!-- ALL-CONTRIBUTORS-LIST:END -->


This project follows the
[all-contributors](https://github.com/all-contributors/all-contributors)
specification. Contributions of any kind welcome!