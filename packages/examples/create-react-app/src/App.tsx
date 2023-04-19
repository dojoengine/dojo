import { DojoConfig, WorldProvider } from "@dojoengine/react"
import { Position } from "./components/Position";
// import manifest from "../../../../examples/target/release/manifest.json"
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
  return (
    <WorldProvider worldAddress={worldAddress} rpcUrl={rpcUrl} connectors={connectors}>
      <div>
        <Connect />
        <GridComponent />
      </div>
    </WorldProvider>
  );
}

export default App;
