import { WorldProvider } from "@dojoengine/react"
// import manifest from "../../../../examples/target/release/manifest.json"
import { Connect } from "./components/Connect";
import ControllerConnector from "@cartridge/connector";
import { InjectedConnector } from "@starknet-react/core";
import GridComponent from "./components/Grid";

const worldAddress = "0x2a79e6863214cfb96bdcb42b70eb39cdb74dd7787d2b1e792b673600892eeb2";
const rpcUrl = "http://127.0.0.1:5050";
const ws = "ws://localhost:9001"


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
    <WorldProvider worldAddress={worldAddress} rpcUrl={rpcUrl} connectors={connectors} ws={ws}>
      <div>
        <Connect />
        <GridComponent />
      </div>
    </WorldProvider>
  );
}

export default App;
