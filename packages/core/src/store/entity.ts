import { createStore } from 'zustand/vanilla'

// store state of entity by their components
// ability to update entity by component state
// return full entity state

interface Component {
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

    // where we call RPC to update state in background

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