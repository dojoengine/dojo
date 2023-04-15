import { RPCProvider } from '../RPCProvider';
// import { RpcProvider } from 'starknet';
import { Query, WorldEntryPoints } from '../../types';


describe('RPCProvider', () => {
    const world_address = '0x0248cacaeac64c45be0c19ee8727e0bb86623ca7fa3f0d431a6c55e200697e5a';

    const rpcUrl = "https://starknet-goerli.cartridge.gg/";

    const rpcProvider = new RPCProvider(world_address, rpcUrl);

    it('should call entity and return the response as an array of bigints', async () => {

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

    it('should fetch multiple entities and return an object of responses', async () => {

        const mockResponse1 = [BigInt(1), BigInt(2), BigInt(3)];
        const mockResponse2 = [BigInt(4), BigInt(5), BigInt(6)];

        rpcProvider.entity = jest.fn()
            .mockResolvedValueOnce(mockResponse1)
            .mockResolvedValueOnce(mockResponse2);

        const parameters = [
            {
                component: "component1",
                query: {
                    partition: "partition1",
                    keys: ["key1", "key2"],
                },
                offset: 0,
                length: 3,
            },
            {
                component: "component2",
                query: {
                    partition: "partition2",
                    keys: ["key3", "key4"],
                },
                offset: 0,
                length: 3,
            },
        ];

        const expectedResult = {
            "component1": mockResponse1,
            "component2": mockResponse2
        };

        const result = await rpcProvider.constructEntity(parameters);

        expect(result).toEqual(expectedResult);
        expect(rpcProvider.entity).toHaveBeenCalledTimes(parameters.length);
    });
});