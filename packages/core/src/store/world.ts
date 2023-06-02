import { createStore } from 'zustand/vanilla'
import { Entity, World as IWorld, Manifest } from '../types';
import { RPCProvider } from '../provider';
import { Account, number } from 'starknet';
import { getEntityComponent, updateComponent, registerEntity } from './entity';

export const worldStore = createStore<IWorld>(() => ({
    world: '',
    executor: '',
    systems: [],
    components: [],
    entities: {},
}))

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

export class World {
    // world
    public world: IWorld;
    private previousComponentData: Map<symbol, any>;
    private optimisticUpdateInfo: Map<symbol, { entityId: number, componentName: string, componentData: any }>;
    private queue: Promise<any>;
    private statuses: Map<symbol, 'idle' | 'loading' | 'done' | 'error'>;

    // public systemCalls: SystemCalls;
    // systems

    constructor(manifest: Manifest) {
        worldStore.setState(state => ({
            world: manifest.world,
            executor: manifest.executor,
            components: manifest.components,
            systems: manifest.systems,
        }))
        this.world = worldStore.getState()
        this.previousComponentData = new Map();
        this.optimisticUpdateInfo = new Map();
        this.queue = Promise.resolve();  // Start the queue
        this.statuses = new Map();
        // this.systemCalls = systemCalls;
    }

    getWorld() {
        return worldStore.getState()
    }

    getWorldAddress() {
        return this.world.world;
    }

    registerEntity(entity: Entity) {
        return registerEntity(entity);
    }

    updateComponent(entityId: number, componentName: string, componentData: any) {
        return updateComponent(entityId, componentName, componentData);
    }

    getEntityComponent(entityId: number, componentName: string) {
        return getEntityComponent(entityId, componentName);
    }

    public prepareOptimisticUpdate(entityId: number, componentName: string, componentData: any): symbol {

        const id = Symbol();
        // Save the previous component data and update information for optimistic update.
        this.previousComponentData.set(id, this.getEntityComponent(entityId, componentName));
        this.optimisticUpdateInfo.set(id, { entityId, componentName, componentData });

        // Optimistically update the component data.
        this.updateComponent(entityId, componentName, componentData);

        return id
    }

    public getCallStatus(id: symbol): 'idle' | 'loading' | 'done' | 'error' {
        return this.statuses.get(id) || 'idle';
    }

    public execute(
        account: Account,
        provider: RPCProvider,
        system: string,
        call_data: number.BigNumberish[],
        id: symbol = Symbol()
    ): symbol {

        // Set the call status to loading.
        this.statuses.set(id, 'loading');

        // Add this execution to the queue.
        this.queue = this.queue.then(() => {
            return this._execute(account, provider, system, call_data, id);
        });

        // Return the unique identifier for the call.
        return id;
    }

    private async _execute(
        account: Account,
        provider: RPCProvider,
        system: string,
        call_data: number.BigNumberish[],
        id: symbol
    ) {
        try {
            // Execute the system call.
            const result = await provider.execute(account, system, call_data);

            // If the system call succeeded, clear the previous component data and optimistic update info.
            this.previousComponentData.delete(id);
            this.optimisticUpdateInfo.delete(id);

            // Set the call status to done.
            this.statuses.set(id, 'done');

            return result;
        } catch (error) {
            // If the system call failed and there was an optimistic update, revert the component data.
            const updateInfo = this.optimisticUpdateInfo.get(id);
            const previousData = this.previousComponentData.get(id);
            if (updateInfo && previousData) {
                const { entityId, componentName } = updateInfo;
                this.updateComponent(entityId, componentName, previousData);
            }

            // Clear the optimistic update info and previous data.
            this.optimisticUpdateInfo.delete(id);
            this.previousComponentData.delete(id);

            // Set the call status to error.
            this.statuses.set(id, 'error');

            throw error;
        }
    }


}
