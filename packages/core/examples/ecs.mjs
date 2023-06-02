import { World } from '../dist/store/index.js';
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

    world.registerEntity(initialEntity)

    console.log(world);

    const componentName = 'Position';
    const componentData = { x: 10, y: 20 };

    const id = world.prepareOptimisticUpdate(initialEntity.id, componentName, componentData);

    // loop to 100
    for (let i = 0; i < 100; i++) {
        world.execute(account.account, rpcProvider, 'Spawn', [], id)
    }

    setTimeout(() => {
        console.log(world.getCallStatus(id));  // 'done' or 'error'
    }, 10);

    console.log(initialEntity.id, world.getEntityComponent(initialEntity.id, 'Position'))

};

main();