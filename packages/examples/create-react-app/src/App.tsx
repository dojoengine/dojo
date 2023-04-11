import { DojoConfig } from "dojo-react"
import { Position } from "./components/Position";
import ControllerConnector from "@cartridge/connector";
import {
  InjectedConnector,
  StarknetProvider,
} from "@starknet-react/core";
import manifest from "../../../../examples/target/release/manifest.json"
import { Connect } from "./components/Connect";

const worldAddress = "0x0248cacaeac64c45be0c19ee8727e0bb86623ca7fa3f0d431a6c55e200697e5a";
const rpcUrl = "https://starknet-goerli.cartridge.gg/";

export const controllerConnector = new ControllerConnector([
  {
    target: worldAddress,
    method: "execute",
  },
]);

export const argentConnector = new InjectedConnector({
  options: {
    id: "argentX",
  },
});

export const connectors = [controllerConnector as any, argentConnector];

function App() {
  const entity_ids = [
    "1", "2", "3"
  ]

  return (
    <div>
      <StarknetProvider connectors={connectors}>
        <DojoConfig worldAddress={worldAddress} rpcUrl={rpcUrl}>
          <h3>State</h3>
          <Connect />

          <div>
            {entity_ids.map((entity_id, index) => (
              <Position key={index} entity_id={entity_id} />
            ))}
          </div>
        </DojoConfig>
      </StarknetProvider>
    </div>

  );
}

export default App;
