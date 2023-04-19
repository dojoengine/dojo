import { useConnectors } from "@starknet-react/core"
import { useWorldContext } from "@dojoengine/react/dist/provider"
import { useEffect } from "react"

export function Connect() {
    const { connect } = useConnectors()
    const { connectors } = useWorldContext()

    return (
        <ul>
            {connectors.map((connector) => (
                <li key={connector.id()}>
                    <button onClick={() => connect(connector)}>
                        Connect {connector.id()}
                    </button>
                </li>
            ))}
        </ul>
    )
}