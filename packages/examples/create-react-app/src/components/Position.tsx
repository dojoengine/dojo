import { useEffect, useState } from "react";
import { useComponent, useSystem, useDojo } from "@dojoengine/react"
import { Position as PositionType } from "../types";
import { PositionParser as parser } from "../parsers";

interface Props {
  entity_id: string;
  src: string;
  direction: number;
  position: {
    x: number;
    y: number;
  };
}


const componentStruct = {
  name: "Position",
  offset: 0,
  length: 0,
}

// takes directional input
const system = {
  name: "Movement"
}

export const Position = ({ entity_id, src, position, direction }: Props) => {

  const {
    useComponent: { component, getComponent },
    useSystem: { execute }
  } = useDojo({ key: "1", parser, optimistic: false });


  useEffect(() => {
    console.log("Moving ", entity_id, " to ", position.x, position.y)

    // execute direction
    execute(
      [BigInt(direction)],
      system.name
    )
  }, [position, entity_id, execute]);

  // get component
  useEffect(() => {
    getComponent(entity_id,
      {
        partition: componentStruct.name,
        keys: [""]
      }
    );
  }, [entity_id])

  return (
    <img
      src={src}
      alt={entity_id}
      style={{ width: '100%', height: '100%', objectFit: 'contain' }}
    />
  );
};

