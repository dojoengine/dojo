`cargo run --bin dojo init`

### Note
This game is designed around 4 elements, Player, Beast, Loot and a low level combat system that can be infinitely reworked. The following game is just one example. 

# Loot Survivors
A game about survival in an infinitely deep world. Players are born naked and without purpose. Through exploration and cunning, they survive and perhaps reach the elusive high score.

Players start off at level 1, and as they progress through the world, they increase their level. Every level they increase, the game becomes more dangerous. Players must choose to purchase the right weapons and engage in the right fights, or they will die. And when dead, they are dead...forever. Players must start their journey again.

Each level affects the beasts and traps the players encounter. There is no limit to the level a player can reach, since there is no hard cap on the level. The game just multiplies in difficulty the deeper it gets.

### Core loop
There is an `explore` system that calculates what the player will encounter from a random number. In the first iteration this will be the only thing a player can do in terms of exploration. As the game is developed we will introduce different Biomes or dungeons.

### Loot items
Loot items are the key factor in this game. They are based off the OG Loot contract and all have deeply intersting statistics, you could call it the loot physics layer. Loot items can be purchased from a generative market every 6 hours.

### Beasts
Beasts can be encounted in the `explore` function. Once discovered you are able to fight Beasts. Each beast has different Armour and Attack types, so you must use the right weapons and armour to defeat. If you are successful, the beast will drop Gold. This Gold is needed for every purchase in the game

### Gold
Gold allows purchasing of Health and of Loot Items from the Market.

TODO:

Entites
1. Adventurer - The player controlled entites in the world
2. Beasts - The enemies in the world
3. Loot - The items in the world

Components
- [ ] Health
- [ ] Gold
- [ ] Experience
- [ ] Entity Statistics
- [ ] Item Statistics

Systems
- [ ] Adjust Health
- [ ] Adjust Gold Balance
- [ ] Explore
- [ ] Birthing
- [ ] Permissions
- [ ] Combat