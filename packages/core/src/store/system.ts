import { worldStore } from './world'
import { RegisteredSystem } from '../types'

export const registerSystem = (systems: RegisteredSystem) => {
    worldStore.setState(state => ({
        ...state,
        systems: [
            ...state.systems,
            systems
        ]
    }))
}

export const getSystems = () => {
    return worldStore.getState().systems;
}

export const getSystem = (name: string) => {
    const systems = worldStore.getState().systems;

    for (let key in systems) {
        if (systems[key].name === name) {
            return systems[key];
        }
    }
    return null;
}