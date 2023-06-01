import Store from '../dist/store/index.js';
import { RPCProvider } from '../dist/provider/RPCProvider.js';
import Manifest from './manifest.json' assert { type: 'json' };
import { HotAccount } from '../dist/account/index.js'

const WorldContractAddress = '0x3f4b4b87bb1c2e6ce758ab610fa2b73cbd1afe554d1d70701b1545b1f29220c';
const address = "0x06f62894bfd81d2e396ce266b2ad0f21e0668d604e5bb1077337b6d570a54aea"
const privateKey = "0x07230b49615d175307d580c33d6fda61fc7b9aec91df0f5c1a5ebe3b8cbfee02"

const url = 'http://127.0.0.1:5050';

const main = async () => {

    const rpcProvider = new RPCProvider(WorldContractAddress, url);

    const account = new HotAccount(rpcProvider.sequencerProvider, address, privateKey)

    Store.registerWorld(Manifest);

    console.log(Store.getWorld());

    const position = Store.getComponent('Position');


    console.log(position);

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
    const newEntity = {
        id: 42312312,
        components: {
            Position: {
                name: 'Position',
                data: { x: 2, y: 0 }
            },
            Velocity: {
                name: 'Velocity',
                data: { x: 1, y: 1 }
            }
        }
    }

    Store.registerEntity(initialEntity);

    Store.registerEntity(newEntity);

    console.log(Store.getWorld());

    // // update state
    Store.updateComponent(newEntity.id, 'Position', { x: 10, y: 20 });

    const entity1 = Store.getWorld().entities[initialEntity.id];
    console.log('Entities:', entity1);

    console.log(newEntity.id, Store.getEntityComponent(newEntity.id, 'Position'))

    try {
        await Store.execute(account.account, rpcProvider, 'Spawn', { x: 30, y: 40 }, [], newEntity.id, true);
    } catch (error) {
        console.error('An error occurred:', error);
    }

    console.log(newEntity.id, Store.getEntityComponent(newEntity.id, 'Position'))

};

main();