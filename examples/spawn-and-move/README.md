# Dojo ECS Example

This repo contains a simple example of using the Dojo ECS system. It defines a simple game where a player has a limited set of moves and can move their position through executing the move system.

## Getting started

```sh
# Build the world
sozo build

# Migrate the world
sozo migrate

# Get the class hash of the Moves model by name
sozo model class-hash --world 0x26065106fa319c3981618e7567480a50132f23932226a51c219ffb8e47daa84 Moves
> 0x2b97f0b24be59ecf4504a27ac2301179be7df44c4c7d9482cd7b36137bc0fa4

# Get the schema of the Moves model
sozo model schema --world 0x26065106fa319c3981618e7567480a50132f23932226a51c219ffb8e47daa84 Moves
> struct Moves {
>    remaining: u8
> }

# Get the value of the Moves model for an entity. (in this example,
# 0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973 is
# the calling account.
sozo model get --world 0x26065106fa319c3981618e7567480a50132f23932226a51c219ffb8e47daa84 Moves 0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973
> 0x0

# The returned value is 0 since we haven't spawned yet. 
# We can spawn a player using the actions contract address
sozo execute 0x31571485922572446df9e3198a891e10d3a48e544544317dbcbb667e15848cd spawn

# Fetch the updated entity
sozo model get --world 0x26065106fa319c3981618e7567480a50132f23932226a51c219ffb8e47daa84 Moves 0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973
> 0xa
```
