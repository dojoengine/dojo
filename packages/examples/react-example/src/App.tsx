import { useEffect } from 'react';
import './App.css'
import { useDojo } from './DojoContext'
import { useComponentValue } from "@dojoengine/react";
import { Query } from '@dojoengine/core';

function App() {

  const {
    systemCalls: { spawn },
    components: { Position, Moves },
    network: { world, signer, entity }
  } = useDojo()

  // world.registerEntity({ id: 1 as any })

  const query: Query = { address_domain: "0", partition: "0", keys: [BigInt(signer.address)] }

  async function getEntity() {
    try {
      const va = await entity(Position.metadata.name, query);
      return va
    } catch (e) {
      console.log(e)
    } finally {
      console.log('done')
    }
  }

  useEffect(() => {
    getEntity().then(va => console.log(va));
  }, [])

  return (
    <>
      <div className="card">
        <button onClick={() => spawn()}>
          spawn
        </button>
      </div>
    </>
  )
}

export default App
