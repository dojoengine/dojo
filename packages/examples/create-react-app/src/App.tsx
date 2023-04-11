import { DojoConfig, WorldProvider } from "dojo-react"
import { Position } from "./components/Position";
import manifest from "../../../../examples/target/release/manifest.json"
import { Connect } from "./components/Connect";
import ControllerConnector from "@cartridge/connector";
import { InjectedConnector } from "@starknet-react/core";
import GridComponent from "./components/Grid";

const worldAddress = "0x0248cacaeac64c45be0c19ee8727e0bb86623ca7fa3f0d431a6c55e200697e5a";
const rpcUrl = "https://starknet-goerli.cartridge.gg/";
// might need to pass this in as a prop
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

function App() {

  // todo: fetch entties from the world
  const entities = [
    { id: 'A', src: 'player.png', position: { x: 10, y: 10 } },
    { id: 'B', src: 'player.png', position: { x: 20, y: 20 } },
    { id: 'C', src: 'player.png', position: { x: 30, y: 30 } },
    { id: 'D', src: 'nazi.png', position: { x: 2, y: 3 } },
    { id: 'E', src: 'nazi.png', position: { x: 12, y: 33 } },
    { id: 'F', src: 'nazi.png', position: { x: 4, y: 34 } },
  ];


  return (
    <WorldProvider worldAddress={worldAddress} rpcUrl={rpcUrl} connectors={connectors}>
      <div>
        <GridComponent entities={entities} />
        <h3>State</h3>
        <Connect />
        {/* <div>
          {entity_ids.map((entity_id, index) => (
            <Position key={index} entity_id={entity_id} />
          ))}
        </div> */}
      </div>
    </WorldProvider>
  );
}

export default App;
