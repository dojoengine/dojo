import { createStore } from 'zustand/vanilla'
import { subscribeWithSelector } from 'zustand/middleware'
import { ComponentNames, ComponentQuery, Entity, ExecuteState, World as IWorld, Manifest, Query, SystemNames } from '../types';
import { RPCProvider } from '../provider';
import { Account, number } from 'starknet';
import { getEntityComponent, updateComponent, registerEntity, queryEntities, queryEntitiesByValues, removeComponent, deleteEntity } from './entity';
import { HotAccount } from '../account';
import { KATANA_ACCOUNT_1_ADDRESS, KATANA_ACCOUNT_1_PRIVATEKEY, LOCAL_TORII } from '../constants';

export const worldStore = createStore(subscribeWithSelector<IWorld>(() => ({
    world: '',
    executor: '',
    systems: [],
    components: [],
    entities: {},
})))

// TODO: Get entity entity

export class World {
    public provider: RPCProvider;
    public account: Account;
    public world: IWorld;
    private previousComponentData: Map<symbol, any>;
    private optimisticUpdateInfo: Map<symbol, { entityId: number, componentName: string, componentData: any }>;
    private queue: Promise<any>;
    private statuses: Map<symbol, ExecuteState>;

    constructor(manifest: Manifest, account?: Account, provider?: RPCProvider) {
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

        this.provider = provider || new RPCProvider(manifest.world, LOCAL_TORII);
        this.account = account || new HotAccount(this.provider.sequencerProvider, KATANA_ACCOUNT_1_ADDRESS, KATANA_ACCOUNT_1_PRIVATEKEY).account

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

    deleteEntity(entityId: number) {
        return deleteEntity(entityId);
    }

    updateComponent(entityId: number, componentName: string, componentData: any) {
        return updateComponent(entityId, componentName, componentData);
    }

    getEntityComponent(entityId: number, componentName: string) {
        return getEntityComponent(entityId, componentName);
    }

    getComponentValue(component: ComponentNames, query: Query, offset: number = 0, length: number = 0) {
        return this.provider.entity(component, query, offset, length);
    }

    getEntitiesByComponent(...components: ComponentNames[]) {
        return queryEntities(...components)
    }

    getEntitiesByComponentValue(...componentQueries: ComponentQuery[]) {
        return queryEntitiesByValues(...componentQueries);
    }

    removeComponent(entityId: number, componentName: string) {
        return removeComponent(entityId, componentName);
    }

    prepareOptimisticUpdate(entityId: number, componentName: string, componentData: any): symbol {

        const id = Symbol();
        // Save the previous component data and update information for optimistic update.
        this.previousComponentData.set(id, this.getEntityComponent(entityId, componentName));
        this.optimisticUpdateInfo.set(id, { entityId, componentName, componentData });

        // Optimistically update the component data.
        this.updateComponent(entityId, componentName, componentData);

        return id
    }

    getCallStatus(id: symbol): ExecuteState {
        return this.statuses.get(id) || 'idle';
    }

    execute(
        system: SystemNames,
        call_data: number.BigNumberish[],
        id: symbol = Symbol()
    ): symbol {

        // Set the call status to loading.
        this.statuses.set(id, 'loading');

        // Add this execution to the queue.
        this.queue = this.queue.then(() => {
            return this._execute(this.account, this.provider, system, call_data, id);
        });

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
