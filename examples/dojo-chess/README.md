# Dojo Chess Example

This repo contains a simple chess game of using the Dojo ECS system. It also linked to Dojo book [tutorial](https://book.dojoengine.org/tutorial/onchain-chess/index.html).

## Getting started

```sh
# Build the world
sozo build

# Migrate the world
katana
sozo migrate

# Test the world contracts
sozo test
```

## Architecture

### Components, Entity

We have each piece as a seperate entity

- White pawn 1 ( Entity )
  - Piece ( Component )
  - Position ( Component )

We have Game entity with auth

- Game 1 ( Entity )
  - Game ( Component )
  - GameTurn ( Component )
  - PlayersId ( Component )

### System

- Initiate ( System )

  - Initiate Game
    - Generate Game Enitity
  - Initiate Pieces

- Execute Move ( System )

  - Generate Board Cache
  - Generate Possible moves
    - If there is piece need to occupy, kill piece
  - Check if next position is eligible to moves
  - Check Piece is owned by caller
  - Check is caller's turn
  - Update the position of the piece

- Give up ( System )
  - Check caller's color and set winner of opponent
