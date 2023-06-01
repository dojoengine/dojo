import { createStore } from 'zustand/vanilla'

// store state of entity by their components
// ability to update entity by component state
// return full entity state

export interface Component {
    name: string;
    data: any;
}

interface Entity {
    id: number;
    components: Record<string, Component>;
}

interface EntityState {
    entities: Record<number, Entity>;
}

export const useEntityStore = createStore<EntityState>(() => ({
    entities: {}
}))

export const registerEntity = (entity: Entity) => {
    useEntityStore.setState(state => ({
        ...state,
        entities: {
            ...state.entities,
            [entity.id]: entity
        }
    }))
}

export const updateComponent = (entityId: number, componentName: string, componentData: any) => {

    useEntityStore.setState(state => {
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
    const state = useEntityStore.getState();
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