import { RegisteredComponent } from '../types'
import { world } from './world'

export const registerComponent = (components: RegisteredComponent) => {
    world.setState(state => ({
        ...state,
        components: [
            ...state.components,
            components
        ]
    }))
}

export const getComponents = () => {
    return world.getState().components;
}

export const getComponent = (name: string) => {
    const components = world.getState().components;

    for (let key in components) {
        if (components[key].name === name) {
            return components[key];
        }
    }
    return null;
}