import Button from "../elements/Button";
import { useDojoEntity } from "../../../../react/src/hooks/index"

interface Props {
  entity_id: string;
}

const component = {
  component: "0x000",
  offset: 2,
  length: 1,
}

export const Realm = ({ entity_id }: Props) => {

  // dummy key, this can be added in from the world to update state on change
  const { entity, getEntity } = useDojoEntity(2); 
  
  const Realm = getEntity(BigInt(component.component), { partition: entity_id, keys: [""] }, component.offset, component.length);

  return (
    <div>
      {/* <h1 className="text-white">{Realm?.name}</h1> */}
      {/* <Button onClick={() => rpcProvider?.set_entity()}>Realm</Button> */}
    </div>
  );
};
