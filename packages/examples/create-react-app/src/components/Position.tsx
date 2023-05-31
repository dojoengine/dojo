import { useEffect } from "react";
import { useDojo } from "@dojoengine/react"
import { PositionParser as parser } from "../parsers";
import { Account, ec, Provider, stark, number } from "starknet";


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
  id: "0x48470d2bdb97afe267b7d7fd4fb485568e7a9151dcea3d02eeedcc4ed3d36c3",
  offset: 0,
  length: 0,
}

// takes directional input
const system = {
  name: "0x7463417c058526917303293d161f7c2bd6bb0e3f69aa521206b7db03fc56784"
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
    console.log("component", component)
    execute(
      [number.toBN(1), number.toBN(direction)],
      system.name
    )

    // if (stream) console.log(stream)

  }, [position, entityId, execute]);


  useEffect(() => {
    fetch(componentStruct.id,
      {
        partition: "0x06f62894bfd81d2e396ce266b2ad0f21e0668d604e5bb1077337b6d570a54aea",
        keys: [""]
      }
    );
  }, [entityId])

  return (
    <div>
      <img
        src={src}
        alt={entityId}
        style={{ width: '100%', height: '100%', objectFit: 'contain' }}
      />
    </div>
  );
};

