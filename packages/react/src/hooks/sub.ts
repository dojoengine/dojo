import { useEffect } from "react";
import { useWebSocketContext } from "../provider/WSProvider"

export function useWebSocket<T>({
    entityId,
    componentId,
    parser,
    store
}: {
    entityId: string;
    componentId: string;
    parser: (data: any) => T | undefined;
    store: any;
}) {
    const { addMessageListener, removeMessageListener } = useWebSocketContext();

    useEffect(() => {
        const messageListener = (message: any) => {
            if (message.entityId === entityId && message.componentId === componentId) {
                // Update the store with the received data
                store.setState({ value: message });
            }
        };

        // Add the message listener
        addMessageListener(messageListener);

        // Cleanup on unmount
        return () => {
            removeMessageListener(messageListener);
        };
    }, [entityId, componentId, addMessageListener, removeMessageListener, store]);

    return {
        component: parser(store.getState().value),
    };
}