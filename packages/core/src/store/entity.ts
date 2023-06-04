import { worldStore } from './world'
import { Entity } from '../types';

// store state of entity by their components
// ability to update entity by component state
// return full entity state

// TODO: Currently two types of components - world registered, then component data for entities
export interface Component {
    name: string;
    data: any;
}

export const registerEntity = (entity: Entity) => {
    worldStore.setState(state => ({
        ...state,
        entities: {
            ...state.entities,
            [entity.id]: entity
        }
    }))
}

export const updateComponent = (entityId: number, componentName: string, componentData: any) => {
    worldStore.setState(state => {
        const entity = state.entities[entityId];
        if (!entity) {
            console.error(`Entity with ID ${entityId} not found.`);
            return state;
        }

        return {
            ...state,
            entities: {
                ...state.entities,
                [entityId]: {
                    ...entity,
                    components: {
                        ...entity.components,
                        [componentName]: {
                            name: componentName,
                            data: componentData
                        }
                    }
                }
            }
        }
    })
}

export const getEntityComponent = (entityId: number, componentName: string): Component | undefined => {
    const state = worldStore.getState();
    const entity = state.entities[entityId];

    if (!entity) {
        console.error(`Entity with ID ${entityId} not found.`);
        return undefined;
    }

    const component = entity.components[componentName];

    if (!component) {
        console.error(`Component with name ${componentName} not found in entity ${entityId}.`);
        return undefined;
    }

    return component;
}