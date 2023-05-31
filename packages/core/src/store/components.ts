import { createStore } from 'zustand/vanilla'
import { Component, ComponentNames } from '../types'

export const ComponentsStore = createStore<Component[]>(() => ([{
    name: '',
    members: [],
    class_hash: ''
}]))

/**
 * 
 * @param name array of component names
 */
export const registerComponent = (components: Component) => {
    ComponentsStore.setState(state => ([
        ...state,
        components
    ]))
}

// remove components

/**
 *  @returns array of component names
*/
export const getComponents = () => {
    return ComponentsStore.getState()
}

/**
 * @param {ComponentNames} name
 * @returns {Object} component
 */
export const getComponent = (name: string) => {
    const components = ComponentsStore.getState();

    for (let key in components) {
        if (components[key].name === name) {
            return components[key];
        }
    }
    return null;
}