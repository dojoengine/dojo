import { SetupNetworkResult } from "./setupNetwork";

export type SystemCalls = ReturnType<typeof createSystemCalls>;

export enum Direction {
    Left = 1,
    Right = 2,
    Up = 3,
    Down = 4,
}

export function createSystemCalls(
    { execute, syncWorker }: SetupNetworkResult,
) {
    const spawn = async () => {
        const tx = await execute("spawn", []);
        // await awaitStreamValue(txReduced$, (txHash) => txHash === tx.transaction_hash);
        syncWorker.sync(tx.transaction_hash);
        
    };

    const move = async (direction: Direction) => {
        // execute from core
        const tx = await execute("move", [direction]);
        // awaitStreamValue(txReduced$, (txHash) => txHash === tx.transaction_hash);
        syncWorker.sync(tx.transaction_hash);
      };


    return {
        spawn,
        move
    };
}