import { InjectedConnector, useConnectors } from "@starknet-react/core"
import { useWorldContext } from "@dojoengine/react/dist/provider"
import ControllerConnector from "@cartridge/connector";

const worldAddress = "0x0248cacaeac64c45be0c19ee8727e0bb86623ca7fa3f0d431a6c55e200697e5a";

const controllerConnector = new ControllerConnector([
    {
        target: worldAddress,
        method: "execute",
    },
]);

const argentConnector = new InjectedConnector({
    options: {
        id: "argentX",
    },
});

const connectors = [controllerConnector as any, argentConnector];


export function Connect() {
    const { connect } = useConnectors()

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