import { useEffect, useState } from "react";
import { useDojoEntity } from "dojo-react"
import { Position as PositionType } from "../types";

import { PositionParser as parser } from "../parsers";

interface Props {
  entity_id: string;
}

const component = {
  component: "0x000",
  offset: 2,
  length: 1,
}

export const Position = ({ entity_id }: Props) => {

  const [counter, setCounter] = useState(0);

  const { entity, getEntity, setEntity } = useDojoEntity<PositionType>({ key: 1, parser });

  useEffect(() => {
    // getEntity(BigInt(component.component), { partition: entity_id, keys: [''] }, component.offset, component.length);

    console.log("entity", entity);
  }, [entity_id, getEntity, counter]);

  if (!entity) {
    return <div>Loading...</div>;
  }

  const pos = { entity: [BigInt("1"), BigInt("3")] }

  return (
    <div>
      <h4>Position</h4>
      <p>Entity ID: {entity_id}</p>
      <p>[{entity.x && entity.x.toString()}, {entity.y && entity.y.toString()}]</p>
      <button onClick={() => {
        setEntity(pos)
        setCounter(counter + 1);
      }}>Set Coordinates</button>
    </div>
  );
};