import { RPCProvider } from '../RPCProvider';
import { Query, WorldEntryPoints } from '../../types';
import { DOJO_STARTER_WORLD, LOCAL_KATANA } from '../../constants';


describe('RPCProvider', () => {

    const rpcProvider = new RPCProvider(DOJO_STARTER_WORLD, LOCAL_KATANA);

    it('should call entity and return the response as an array of bigints', async () => {

        const mockResponse = {
            result: [BigInt(1), BigInt(1), BigInt(1)],
        };
        rpcProvider.entity = jest.fn().mockResolvedValue(mockResponse);

        const component = 'Position';
        const query: Query = { address_domain: '0', keys: [BigInt(1), BigInt(1)] };
        const offset = 0;
        const length = 3;

        const result = await rpcProvider.entity(component, query, offset, length);

        console.log(result)

        expect(result).toEqual(mockResponse);
    });
});