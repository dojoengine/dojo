import { World } from '../dist/store/index.js';
import Manifest from './manifest.json' assert { type: 'json' };

const main = async () => {

    // creates world from manifest
    // we don't pass any Account or RPC, the Class comes with the Katana default RPC
    const world = new World(Manifest);

    const initialEntity = {
        id: 213123121,
        components: {
            Position: {
                name: 'Position',
                data: { x: 0, y: 0 }
            },
            Velocity: {
                name: 'Velocity',
                data: { x: 1, y: 1 }
            }
        }
    }
    for (let i = 0; i < 100; i++) {
        world.registerEntity({
            id: 213123121 + i,
            components: {
                Position: {
                    name: 'Position',
                    data: { x: 0, y: 0 }
                },
                Velocity: {
                    name: 'Velocity',
                    data: { x: 1, y: 1 }
                }
            }
        })
    }
    console.log(world);

    const keys = [BigInt(world.getWorldAddress())]

    const query = { address_domain: 0, partition: 0, keys: keys };

    const componentName = 'Position';

    const componentData = { x: 10, y: 20 };

    const id = world.prepareOptimisticUpdate(initialEntity.id, componentName, componentData);

    // loop to 100
    for (let i = 0; i < 100; i++) {
        await world.execute('Spawn', [], id)
    }

    setTimeout(() => {
        console.log(world.getCallStatus(id));  // 'done' or 'error'
    }, 10);

    // console.log(initialEntity.id, world.getEntityComponent(initialEntity.id, componentName))

    const movesValue = await world.getComponentValue(componentName, query)

    // console.log(movesValue)

    const componentsWithPosition = world.getEntitiesByComponent('Position', 'Velocity');

    // console.log(componentsWithPosition)

    const entitiesWithSpecificValues = world.getEntitiesByComponentValue(
        { name: 'Position', dataValues: { x: 10, y: 20 } }
    );

    console.log("entitiesWithSpecificValues", entitiesWithSpecificValues)

};

main();