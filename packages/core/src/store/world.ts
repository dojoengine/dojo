import { createStore } from 'zustand/vanilla'
import { World, Manifest } from '../types';
import { RPCProvider } from '../provider';
import { Account, number } from 'starknet';
import { getEntityComponent, updateComponent } from './entity';


export const world = createStore<World>(() => ({
    world: '',
    executor: '',
    systems: [],
    components: [],
    entities: {},
}))

/**
 * @param manifest dojo manifest
 * @returns
*/
export const registerWorld = (manifest: Manifest) => {
    world.setState(state => ({
        world: manifest.world,
        executor: manifest.executor,
        components: manifest.components,
        systems: manifest.systems,
    }))
}

/**
 *  @returns world  
*/
export const getWorld = () => {
    return world.getState()
}

// TODO: clean params
export async function execute(
    account: Account,
    provider: RPCProvider,
    system: string,
    component_data: any,
    call_data: number.BigNumberish[],
    entity_id: number,
    optimistic: boolean = false
) {

    // TODO: check system registered

    // get current entity by component
    const entity = getEntityComponent(entity_id, 'Position');

    // set component Store for Optimistic UI
    if (optimistic) updateComponent(entity_id, 'Position', component_data);

    try {
        const result = await provider.execute(account, system, call_data);

        return result;
    } catch (error) {
        // revert state if optimistic
        if (optimistic && entity) updateComponent(entity_id, system, entity.data);

        throw error;
    }
}

