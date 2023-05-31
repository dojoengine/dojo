import { InjectedConnector, useConnectors } from "@starknet-react/core"
import { useWorldContext } from "@dojoengine/react/dist/provider"
import ControllerConnector from "@cartridge/connector";
import { useDojo } from "@dojoengine/react"
import { PositionParser as parser } from "../parsers";
import { Account, ec, Provider, stark, number } from "starknet";

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

const entityId = "0x06f62894bfd81d2e396ce266b2ad0f21e0668d604e5bb1077337b6d570a54aea"

const componentStruct = {
    id: "Position",
    offset: 0,
    length: 0,
}

// takes directional input
const system = {
    name: "0x287805587e3abd3111e19b56dbe7d8b8458e3ffb95a8f272466c1072b80e519"
}

export function Connect() {
    const { connect } = useConnectors()

    const params = {
        key: "1",
        parser,
        // componentState: [BigInt(position.x), BigInt(position.y)],
        componentId: componentStruct.id,
        entityId
    }

    const {
        component,
        fetch,
        execute,
        stream
    } = useDojo(params);

    return (
        <div>
            <button onClick={() => execute([], system.name)}>spawn</button>
        </div>
        // <ul>
        //     {connectors.map((connector) => (
        //         <li key={connector.id()}>
        //             <button onClick={() => connect(connector)}>
        //                 Connect {connector.id()}
        //             </button>
        //         </li>
        //     ))}
        // </ul>
    )
}