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
    const { sendMessage, addMessageListener, removeMessageListener } = useWebSocketContext();

    useEffect(() => {

        // TODO: Types
        const messageListener = (message: any) => {
            // console.log(entityId, "Received message:", message);
            // if (message.entityId === entityId && message.componentId === componentId) {
            // Update the store with the received data
            store.setState({ value: message.data });

            // console.log(entityId, message.data)
            // }
        };

        // Add the message listener
        addMessageListener(messageListener);

        // Cleanup on unmount
        return () => {
            removeMessageListener(messageListener);
        };
    }, [entityId, componentId, addMessageListener, removeMessageListener, store]);

    return {
        stream: parser(store.getState().value),
    };
}