import { createStore } from 'zustand/vanilla'
import { System } from '../types'


export const SystemsStore = createStore<System>(() => ({
    name: '',
    inputs: [],
    outputs: [],
    class_hash: '',
    dependencies: [],
}))

export const registerSystem = (systems: System[]) => {
    SystemsStore.setState(state => ({
        ...state,
        ...systems
    }))
}