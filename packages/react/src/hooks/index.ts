import { Store } from '@dojoengine/core';

import { useComponent } from './get';
import { useSystem } from './set';

// export indpendent hooks
export { useComponent } from './get';
export { useSystem } from './set';


// export a hook that combines the two and shares a state
export function useDojo<T>({
    key,
    parser,
    optimistic,
    componentState,
}: {
    key: string;
    parser: (data: any) => T | undefined;
    optimistic?: boolean;
    componentState?: bigint[];
}) {
    const store = Store.ComponentStore;

    if (componentState) {
        store.setState({ value: componentState });
    }

    return {
        useComponent: useComponent<T>({ key, parser, store }),
        useSystem: useSystem<T>({ key })
    };
}