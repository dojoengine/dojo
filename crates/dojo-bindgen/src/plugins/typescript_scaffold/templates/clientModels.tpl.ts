{{ header }}

import { ContractComponents } from "./generated/contracts.gen.ts";

export type ClientModels = ReturnType<typeof models>;

export function models({
    contractModels,
}: {
    contractModels: ContractComponents;
}) {
    return {
        models: {
            ...contractModels,
        },
    };
}
