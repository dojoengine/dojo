import { world } from './world'
import { RegisteredSystem } from '../types'

export const registerSystem = (systems: RegisteredSystem) => {
    world.setState(state => ({
        ...state,
        systems: [
            ...state.systems,
            systems
        ]
    }))
}

export const getSystems = () => {
    return world.getState().systems;
}

export const getSystem = (name: string) => {
    const systems = world.getState().systems;

    for (let key in systems) {
        if (systems[key].name === name) {
            return systems[key];
        }
    }
    return null;
}