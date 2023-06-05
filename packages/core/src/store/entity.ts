import { worldStore } from './world'
import { ComponentQuery, Entity } from '../types';

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

export const deleteEntity = (entityId: number) => {
    worldStore.setState(state => {
        // Make a copy of the entities object.
        const newEntities = { ...state.entities };

        // Delete the entity with the provided ID.
        delete newEntities[entityId];

        // Return the updated state.
        return {
            ...state,
            entities: newEntities
        }
    })
}

export const updateComponent = (entityId: number, componentName: string, componentData: any) => {
    worldStore.setState(state => {
        let entity = state.entities[entityId];

        // If no entity exists, create a new one
        if (!entity) {
            entity = {
                id: entityId,
                components: {}
            };
        }

        // Store the component
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
    });
};

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

export const queryEntities = (...components: string[]) => {
    const state = worldStore.getState();

    const entitiesWithComponents = Object.entries(state.entities).filter(([entityId, entity]) => {
        return components.every(componentName => entity.components.hasOwnProperty(componentName));
    });

    const entitiesWithComponentsObject = Object.fromEntries(entitiesWithComponents);

    return entitiesWithComponentsObject;
}

export const queryEntitiesByValues = (...componentQueries: ComponentQuery[]) => {
    const state = worldStore.getState();

    const entitiesWithComponents = Object.entries(state.entities).filter(([entityId, entity]) => {
        return componentQueries.every(query => {
            const component = entity.components[query.name];
            if (!component) {
                return false;
            }

            if (query.dataValues) {
                return Object.entries(query.dataValues).every(([key, value]) => {
                    return component.data[key] === value;
                });
            }

            return true;
        });
    });

    const entitiesWithComponentsObject = Object.fromEntries(entitiesWithComponents);

    return entitiesWithComponentsObject;
}

export const removeComponent = (entityId: number, componentName: string) => {
    worldStore.setState(state => {
        const entity = state.entities[entityId];
        if (!entity) {
            console.error(`Entity with ID ${entityId} not found.`);
            return state;
        }

        const newComponents = { ...entity.components };
        delete newComponents[componentName];

        return {
            ...state,
            entities: {
                ...state.entities,
                [entityId]: {
                    ...entity,
                    components: newComponents
                }
            }
        }
    });
}