# Dojo ECS Example

This repo contains a simple example of using the Dojo ECS system. It defines a simple game where a player has a limited set of moves and can move their position through executing the move system.

## Getting started

```sh
# Build the world
sozo build

# Migrate the world
sozo migrate

# Get the class hash of the Moves model by name
sozo model class-hash Moves --world 0x33ac2f528bb97cc7b79148fd1756dc368be0e95d391d8c6d6473ecb60b4560e
> 0x64495ca6dc1dc328972697b30468cea364bcb7452bbb6e4aaad3e4b3f190147

# Get the schema of the Moves model
sozo model schema Moves --world 0x33ac2f528bb97cc7b79148fd1756dc368be0e95d391d8c6d6473ecb60b4560e
> struct Moves {
>   #[key]
>   player: ContractAddress,
>   remaining: u8,
>   last_direction: Direction = Invalid Option,
> }
>
> enum Direction {
>   None
>   Left
>   Right
>   Up
>   Down
> }

# Get the value of the Moves model for an entity. (in this example,
# 0x6162896d1d7ab204c7ccac6dd5f8e9e7c25ecd5ae4fcb4ad32e57786bb46e03, is
# the calling account which is also the key to retrieve a Moves model)
sozo model get Moves 0x6162896d1d7ab204c7ccac6dd5f8e9e7c25ecd5ae4fcb4ad32e57786bb46e03 --world 0x33ac2f528bb97cc7b79148fd1756dc368be0e95d391d8c6d6473ecb60b4560e
> struct Moves {
>   #[key]
>   player: ContractAddress = 0x6162896d1d7ab204c7ccac6dd5f8e9e7c25ecd5ae4fcb4ad32e57786bb46e03,
>   remaining: u8 = 0,
>   last_direction: Direction = None,
> }

# The returned value is 0 since we haven't spawned yet.
# We can spawn a player using the actions contract address.
sozo execute 0x152dcff993befafe5001975149d2c50bd9621da7cbaed74f68e7d5e54e65abc spawn

# Fetch the updated entity.
sozo model get Moves 0x6162896d1d7ab204c7ccac6dd5f8e9e7c25ecd5ae4fcb4ad32e57786bb46e03 --world 0x33ac2f528bb97cc7b79148fd1756dc368be0e95d391d8c6d6473ecb60b4560e
> struct Moves {
>   #[key]
>   player: ContractAddress = 0x6162896d1d7ab204c7ccac6dd5f8e9e7c25ecd5ae4fcb4ad32e57786bb46e03,
>   remaining: u8 = 1,
>   last_direction: Direction = None,
> }
>
> enum Direction {
>   None
>   Left
>   Right
>   Up
>   Down
> }
```
