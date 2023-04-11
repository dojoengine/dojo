import { useEffect, useState } from "react";
import { useDojoEntity } from "dojo-react"
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

  const { entity, getEntity, setEntity } = useDojoEntity({ key: 1, parser, optimistic: false });

  useEffect(() => {
    getEntity(component.component, { partition: entity_id, keys: [''] }, component.offset, component.length);
  }, [counter]);

  if (!entity) {
    return <div>Loading...</div>;
  }

  const pos = [BigInt(1), BigInt(2)]

  return (
    <div>
      <h4>Position</h4>
      <p>Entity ID: {entity_id}</p>
      <p>[{entity.x && entity.x.toString()}, {entity.y && entity.y.toString()}]</p>
      <button onClick={() => {
        setEntity(pos, component.component, true)
      }}>execute</button>
    </div>
  );
};