import { DojoConfig } from "dojo-react"
import { Position } from "./components/Position";

const worldAddress = "0xK4T4N4";
const rpcUrl = "http://localhost:8545";

function App() {

  const entity_ids = [
    "1", "2", "3"
  ]

  return (
    <div>
      <h3>State</h3>
      <DojoConfig worldAddress={worldAddress} rpcUrl={rpcUrl}>
        <div>
          {entity_ids.map((entity_id, index) => (
            <Position key={index} entity_id={entity_id} />
          ))}
        </div>
      </DojoConfig>
    </div>

  );
}

export default App;
