# Dojo ECS Example

This repo contains a simple example of using the Dojo ECS system. It defines a simple game where a player has a limited set of moves and can move their position through executing the move system.

## Getting started

```sh
# Build the world
sozo build

# Migrate the world
sozo migrate

# Get the class hash of the Moves component by name
sozo component get --world 0xeb752067993e3e1903ba501267664b4ef2f1e40f629a17a0180367e4f68428 Moves
> 0x2b97f0b24be59ecf4504a27ac2301179be7df44c4c7d9482cd7b36137bc0fa4

# Get the schema of the Moves component
sozo component schema --world 0xeb752067993e3e1903ba501267664b4ef2f1e40f629a17a0180367e4f68428 Moves
> struct Moves {
>    remaining: u8
> }

# Get the value of the Moves component for an entity. (in this example,
# 0x3ee9e18edc71a6df30ac3aca2e0b02a198fbce19b7480a63a0d71cbd76652e0 is
# the calling account.
sozo component entity --world 0xeb752067993e3e1903ba501267664b4ef2f1e40f629a17a0180367e4f68428 Moves 0x3ee9e18edc71a6df30ac3aca2e0b02a198fbce19b7480a63a0d71cbd76652e0
> 0x0

# The returned value is 0 since we haven't spawned yet. Let's spawn
# a player for the caller
sozo execute --world 0xeb752067993e3e1903ba501267664b4ef2f1e40f629a17a0180367e4f68428 Spawn

# Fetch the updated entity
sozo component entity --world 0xeb752067993e3e1903ba501267664b4ef2f1e40f629a17a0180367e4f68428 Moves 0x3ee9e18edc71a6df30ac3aca2e0b02a198fbce19b7480a63a0d71cbd76652e0
> 0xa
```