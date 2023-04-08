import { useEffect } from "react";
import { useDojoEntity } from "../../../../react/src/hooks/index"
import { Realm } from "../types";

import { RealmParser as parser } from "../parsers";

interface Props {
  entity_id: string;
}

const component = {
  component: "0x000",
  offset: 2,
  length: 1,
}

export const RealmView = ({ entity_id }: Props) => {

  const { entity, getEntity } = useDojoEntity<Realm>({ key: 2, parser });

  useEffect(() => {
    getEntity(BigInt(component.component), { partition: entity_id, keys: [''] }, component.offset, component.length);
  }, [entity_id, getEntity]);

  if (!entity) {
    return <div>Loading...</div>;
  }

  return (
    <div>
      <h2>{entity.name}</h2>
      <p>{entity.description}</p>
      <p>Owner: {entity.owner}</p>
      <p>Armies: {entity.armies.join(', ')}</p>
    </div>
  );
};