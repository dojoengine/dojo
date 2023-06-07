import { getComponentValue } from "@latticexyz/recs";
import { awaitStreamValue } from "@latticexyz/utils";
import { ClientComponents } from "./createClientComponents";
import { SetupNetworkResult } from "./setupNetwork";

export type SystemCalls = ReturnType<typeof createSystemCalls>;

export function createSystemCalls(
    { execute }: SetupNetworkResult,
    { Moves }: ClientComponents
) {
    const spawn = async () => {

        // execute from core

        const tx = await execute("Spawn", []);
        // await awaitStreamValue(txReduced$, (txHash) => txHash === tx.transaction_hash);
        return getComponentValue(Moves, 1 as any);
    };

    return {
        spawn,
    };
}