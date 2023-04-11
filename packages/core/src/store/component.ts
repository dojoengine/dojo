import { createStore } from 'zustand/vanilla'

type Component = {
    entity: bigint;
    value: bigint[];

};

export const ComponentStore = createStore<Component>(() => ({
    entity: BigInt(0),
    value: [],
}))