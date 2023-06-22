import './App.css';
import { useDojo } from './DojoContext';
import { useComponentValue } from "@dojoengine/react";
import { Direction } from './dojo/createSystemCalls'
import { Utils } from '@dojoengine/core';

function App() {
  const {
    systemCalls: { spawn, move },
    components: { Moves, Position },
  } = useDojo();

  const entityId = BigInt('0x06f62894bfd81d2e396ce266b2ad0f21e0668d604e5bb1077337b6d570a54aea');
  const position = useComponentValue(Position, Utils.getEntityIdFromKeys([entityId]));
  const moves = useComponentValue(Moves, Utils.getEntityIdFromKeys([entityId]));

  return (
    <>
      <div className="card">
        <button onClick={() => spawn()}>Spawn</button>
      </div>
      <div className="card">
        {/* 3 == move up */}
        <button onClick={() => move(Direction.Up)}>Move Up</button>
      </div>
      <div className="card">
        <div>Moves Left: {moves ? `${moves['remaining']}` : 'Need to Spawn'}</div>
      </div>
      <div className="card">
        <div>Position: {position ? `${position['x']}, ${position['y']}` : 'Need to Spawn'}</div>
      </div>
    </>
  );
}

export default App;
