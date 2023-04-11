import { useEffect, useState } from "react";
import { useComponent, useSystem } from "dojo-react"
import { Position as PositionType } from "../types";
import { PositionParser as parser } from "../parsers";

interface Props {
  entity_id: string;
}

const component = {
  component: "Position",
  offset: 0,
  length: 0,
}

export const Position = ({ entity_id }: Props) => {
  const [counter, setCounter] = useState(0);

  const { entity, getEntity } = useComponent<PositionType>({ key: 1, parser, optimistic: false });

  const { execute } = useSystem({ key: 1 });

  useEffect(() => {
    getEntity(component.component, { partition: entity_id, keys: [''] });
  }, [counter]);

  if (!entity) {
    return <div>Loading...</div>;
  }

  return (
    <div>
      <h4>Position</h4>
      <p>Entity ID: {entity_id}</p>

      {/*  */}
      <p>[{entity.x && entity.x.toString()}, {entity.y && entity.y.toString()}]</p>

      {/*  */}
      <button onClick={() => {
        execute([BigInt(1), BigInt(2)], component.component, true)
      }}>execute</button>
    </div>
  );
};