{{ header }}
import { AccountInterface } from "starknet";
import { Entity, getComponentValue } from "@dojoengine/recs";
import {
    getEntityIdFromKeys,
    getEvents,
    setComponentsFromEvents,
} from "@dojoengine/utils";
import { uuid } from "@latticexyz/utils";

import { ClientComponents } from "./clientComponent.ts";
import { ContractComponents } from "./clientModels.ts";
import type { IWorld } from "./world.ts";

export type SystemCalls = ReturnType<typeof systems>;

export function systems({
    client,
    clientModels,
    contractComponents,
}: {
    client: IWorld;
    clientModels: ClientComponents;
    contractComponents: ContractComponents;
}) {
    function actions() {
        {% for system in systems %}
          const {{ system.name }} = async (account: AccountInterface) => {
            throw new Error("Not implemented");
          };

        {% endfor %}
        return { {% for system in systems %} {{ system.name }}, {% endfor %} };
    }

    return {
        actions: actions(),
    };
}
