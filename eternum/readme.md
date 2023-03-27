# Eternum AW

Realms NFTs are the source of truth for the metadata in this world.

`SettleSystem` -> Realm owners call this to init a Realm in the World. This creates an S_Realm NFT, which is an ownership key allowing access to the Realm entity.

`BuildLaborSystem` -> S_Realm owners, can use this to build labor on the Realms. You pass in a resource_id, and the system checks if it exists on the Realm. If it does, labor can be created for that resource.

`HarvestLaborSystem` -> S_Realm owners harvest this to convert balance into useable resources

