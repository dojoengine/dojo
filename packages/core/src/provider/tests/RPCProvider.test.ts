import { RPCProvider } from '../RPCProvider';
// import { RpcProvider } from 'starknet';
import { Query, WorldEntryPoints } from '../../types';


describe('RPCProvider', () => {
    const world_address = '0x123456789abcdef';
    const url = 'https://example.com';

    it('should call entity and return the response as an array of bigints', async () => {
        const rpcProvider = new RPCProvider(world_address, url);
        const mockResponse = {
            result: [1, 2, 3],
        };
        rpcProvider.entity = jest.fn().mockResolvedValue(mockResponse);

        const component = 'testComponent';
        const query: Query = { partition: 'testPartition', keys: ['key1', 'key2'] };
        const offset = 0;
        const length = 3;

        const result = await rpcProvider.entity(component, query, offset, length);

        expect(result).toEqual([BigInt(1), BigInt(2), BigInt(3)]);
    });
});