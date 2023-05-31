import { createStore } from 'zustand/vanilla'
import { ComponentsStore } from './components';
import { World, Manifest } from '../types';
import { SystemsStore } from './system';

export const WorldStore = createStore<World>(() => ({
    world: '',
    executor: ''
}))

/**
 * @param manifest dojo manifest
 * @returns
*/
export const registerWorld = (manifest: Manifest) => {
    WorldStore.setState(state => ({
        world: manifest.world,
        executor: manifest.executor
    }))

    ComponentsStore.setState(state => ({
        ...state,
        ...manifest.components
    }))

    SystemsStore.setState(state => ({
        ...state,
        ...manifest.systems
    }))
}

/**
 *  @returns world address  
*/
export const getWorld = () => {
    return WorldStore.getState()
}