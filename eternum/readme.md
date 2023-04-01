# Eternum AW

Realms NFTs are the source of truth for the metadata in this world.

### Entities

`Realms` : Loot Realms

`Armies` : Army (Armies exist as entites controlled by Realm owners, Goblins/Barbarians or other owners)

`Adventurers` : Characters that exist within the world

`Goblins/Barbarians` : AI Controlled units that spawn on the map

`Roads` : Go from A-B in the world. Speed up travel. Decay.

### Systems

`SettleSystem` : Realm owners call this to init a Realm in the World. This creates an S_Realm NFT, which is an ownership key allowing access to the Realm entity.

`BuildLaborSystem` : S_Realm owners, can use this to build labor on the Realms. You pass in a resource_id, and the system checks if it exists on the Realm. If it does, labor can be created for that resource.

`HarvestLaborSystem` : S_Realm owners harvest this to convert balance into useable resources

### Components

`MetaData` : Global level metadata for an Entity

`Realm` : Realm specific data for the Realm entity

`Army` : Army specific data for the Army entity

`Buildings` : Buildings that can exist on an Entity

`Position` : Position of entity



cargo run --bin dojo -- build eternum