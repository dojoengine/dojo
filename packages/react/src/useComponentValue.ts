// HACK IMPORT FROM https://github.com/latticexyz/mud - ty Lattice!

import {
    Component,
    ComponentValue,
    defineQuery,
    EntityIndex,
    getComponentValue,
    Has,
    isComponentUpdate,
    Metadata,
    Schema,
} from "@latticexyz/recs";
import { useEffect, useState } from "react";

export function useComponentValue<S extends Schema>(
    component: Component<S, Metadata, undefined>,
    entity: EntityIndex | undefined,
    defaultValue: ComponentValue<S>
): ComponentValue<S>;

export function useComponentValue<S extends Schema>(
    component: Component<S, Metadata, undefined>,
    entity: EntityIndex | undefined
): ComponentValue<S> | undefined;

export function useComponentValue<S extends Schema>(
    component: Component<S, Metadata, undefined>,
    entity: EntityIndex | undefined,
    defaultValue?: ComponentValue<S>
) {
    const [value, setValue] = useState(entity != null ? getComponentValue(component, entity) : undefined);

    useEffect(() => {
        // component or entity changed, update state to latest value
        setValue(entity != null ? getComponentValue(component, entity) : undefined);
        if (entity == null) return;

        const queryResult = defineQuery([Has(component)], { runOnInit: false });
        const subscription = queryResult.update$.subscribe((update: any) => {
            if (isComponentUpdate(update, component) && update.entity === entity) {
                const [nextValue] = update.value;
                setValue(nextValue);
            }
        });
        return () => subscription.unsubscribe();
    }, [component, entity]);

    return value ?? defaultValue;
}