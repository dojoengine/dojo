import { DojoConfig, WorldProvider } from "dojo-react"
import { Position } from "./components/Position";
import manifest from "../../../../examples/target/release/manifest.json"
import { Connect } from "./components/Connect";

const worldAddress = "0x0248cacaeac64c45be0c19ee8727e0bb86623ca7fa3f0d431a6c55e200697e5a";
const rpcUrl = "https://starknet-goerli.cartridge.gg/";

function App() {
  const entity_ids = [
    "1", "2", "3"
  ]

  return (
    <WorldProvider worldAddress={worldAddress} rpcUrl={rpcUrl}>
      <div>
        <h3>State</h3>
        <Connect />
        <div>
          {entity_ids.map((entity_id, index) => (
            <Position key={index} entity_id={entity_id} />
          ))}
        </div>
      </div>
    </WorldProvider>
  );
}

export default App;
