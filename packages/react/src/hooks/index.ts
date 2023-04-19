import { Store } from '@dojoengine/core';

import { useComponent } from './get';
import { useSystem } from './set';
import { useWebSocket } from './sub';


// export indpendent hooks
export { useComponent } from './get';
export { useSystem } from './set';
export { useWebSocket } from './sub'


// export a hook that combines the two and shares a state
export function useDojo<T>({
    key,
    parser,
    componentState,
    componentId,
    entityId
}: {
    key: string;
    parser: (data: any) => T | undefined;
    componentState?: bigint[];
    componentId: string;
    entityId: string;
}) {
    const store = Store.ComponentStore;

    // if Component State -> update the component as to act optimistically
    if (componentState) {
        store.setState({ value: componentState });
    }

    const { component, fetch } = useComponent({ key, parser, store });
    const { execute } = useSystem({ key })
    const { stream } = useWebSocket({ entityId, componentId, parser, store })

    return {
        component,
        fetch,
        execute,
        stream
    };
}