import { useEffect } from "react";
import { useDojo } from "@dojoengine/react"
import { PositionParser as parser } from "../parsers";

// TODO: add types for component
interface Props {
  entityId: string;
  src: string;
  direction: number;
  position: {
    x: number;
    y: number;
  };
}

const componentStruct = {
  id: "Position",
  offset: 0,
  length: 0,
}

// takes directional input
const system = {
  name: "Movement"
}

export const Position = ({ entityId, src, position, direction }: Props) => {

  const params = {
    key: "1",
    parser,
    componentState: [BigInt(position.x), BigInt(position.y)],
    componentId: componentStruct.id,
    entityId
  }

  const {
    component,
    fetch,
    execute,
    stream
  } = useDojo(params);

  useEffect(() => {
    console.log("Moving ", entityId, " to ", position.x, position.y)
    execute(
      [BigInt(entityId), BigInt(direction)],
      system.name
    )
  }, [position, entityId, execute]);


  useEffect(() => {
    fetch(entityId,
      {
        partition: componentStruct.id,
        keys: [""]
      }
    );
  }, [entityId])

  return (
    <img
      src={src}
      alt={entityId}
      style={{ width: '100%', height: '100%', objectFit: 'contain' }}
    />
  );
};

