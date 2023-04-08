import { Canvas } from "@react-three/fiber";
import { World } from "./layouts/World";
import {DojoConfig} from "../../../react/src/provider/index"

const worldAddress = "ws://localhost:8080";


function App() {
  return (
    <div className="relative w-screen h-screen bg-black">
      <DojoConfig worldAddress={worldAddress}>
        <World />
        <Canvas className="z-10">
          <ambientLight intensity={0.1} />
          <directionalLight color="red" position={[0, 0, 5]} />
          <mesh>
            <boxGeometry />
            <meshStandardMaterial />
          </mesh>
        </Canvas>
      </DojoConfig>
    </div>
  );
}

export default App;
