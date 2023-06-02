import { RegisteredComponent } from '../types'
import { worldStore } from './world'

export const registerComponent = (components: RegisteredComponent) => {
    worldStore.setState(state => ({
        ...state,
        components: [
            ...state.components,
            components
        ]
    }))
}

export const getComponents = () => {
    return worldStore.getState().components;
}

export const getComponent = (name: string) => {
    const components = worldStore.getState().components;

    for (let key in components) {
        if (components[key].name === name) {
            return components[key];
        }
    }
    return null;
}