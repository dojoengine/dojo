// Define the RPC method and parameters


// Create the RPC request object
export interface RPCRequest {
    entity_id: number;
    component: string;
}

// Function to make the RPC request using Fetch API
export async function fetchRPC({ entity_id, component }: RPCRequest) {

    const method = "view";
    const params = {
        entity_id: entity_id
    };
    const rpcRequest = {
        jsonrpc: "2.0",
        id: 1,
        method: method,
        params: params
    };

    try {
        // call API
        const response = await fetch(`https://goerli.dojonet.io/rpc`, {
            method: "POST",
            headers: {
                "Content-Type": "application/json"
            },
            body: JSON.stringify(rpcRequest)
        });

        if (!response.ok) {
            throw new Error(`HTTP error: ${response.status}`);
        }

        const responseData = await response.json();

        if (responseData.error) {
            throw new Error(`RPC error: ${responseData.error.message}`);
        }

        console.log("RPC response:", responseData.result);
    } catch (error) {
        console.error("Error:", error);
    }
}
