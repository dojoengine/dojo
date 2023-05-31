import { RPCProvider } from '../dist/provider/RPCProvider.js';
import { HotAccount } from '../dist/account/index.js'

const WorldContractAddress = '0x2a79e6863214cfb96bdcb42b70eb39cdb74dd7787d2b1e792b673600892eeb2';
const address = "0x06f62894bfd81d2e396ce266b2ad0f21e0668d604e5bb1077337b6d570a54aea"
const privateKey = "0x07230b49615d175307d580c33d6fda61fc7b9aec91df0f5c1a5ebe3b8cbfee02"

const url = 'http://127.0.0.1:5050';

const main = async () => {

    const rpcProvider = new RPCProvider(WorldContractAddress, url);

    const account = new HotAccount(rpcProvider.sequencerProvider, address, privateKey)

    const keys = [BigInt(WorldContractAddress)]

    const query = { address_domain: 0, partition: 0, keys: keys };
    const offset = 0;
    const length = 2;

    // Spawn
    try {
        const response = await rpcProvider.execute(account.account, 'Spawn', []);
        console.log('Response:', response);
    } catch (error) {
        console.error('An error occurred:', error);
    }

    // Position
    try {
        const response = await rpcProvider.entity('Position', query, offset, length);
        console.log('Response:', response);
    } catch (error) {
        console.error('An error occurred:', error);
    }

    try {
        const response = await rpcProvider.component('Position');
        console.log('Response:', response);
    } catch (error) {
        console.error('An error occurred:', error);
    }
};

main();