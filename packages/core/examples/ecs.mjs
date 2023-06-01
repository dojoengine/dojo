import Store from '../dist/store/index.js';
import Manifest from './manifest.json' assert { type: 'json' };

const main = async () => {

    Store.registerWorld(Manifest);

    console.log(Store.getWorld());

    const position = Store.getComponent('Position');


    console.log(position);

    const initialEntity = {
        id: 213123121,
        components: {
            position: {
                name: 'Position',
                data: { x: 0, y: 0 }
            },
            velocity: {
                name: 'Velocity',
                data: { x: 1, y: 1 }
            }
        }
    }
    const newEntity = {
        id: 42312312,
        components: {
            position: {
                name: 'Position',
                data: { x: 0, y: 0 }
            },
            velocity: {
                name: 'Velocity',
                data: { x: 1, y: 1 }
            }
        }
    }

    Store.registerEntity(initialEntity);

    Store.registerEntity(newEntity);

    // update state
    Store.updateComponent(1, 'position', { x: 10, y: 20 });

    const entity1 = Store.useEntityStore.getState();
    console.log('Entities:', entity1);



};

main();