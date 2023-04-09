import { createStore } from 'zustand/vanilla'

type EntityState = {
    entity: bigint[];
};

export const EntityStore = createStore<EntityState>(() => ({
    entity: [],
}))