import { defineContractComponents } from "./contractComponents";
import { world } from "./world";
import { number } from 'starknet';

import { Providers, Query, SyncWorker } from "@dojoengine/core";
import { Account, ec } from "starknet";

export const KATANA_ACCOUNT_1_ADDRESS = "0x06f62894bfd81d2e396ce266b2ad0f21e0668d604e5bb1077337b6d570a54aea"
export const KATANA_ACCOUNT_1_PRIVATEKEY = "0x07230b49615d175307d580c33d6fda61fc7b9aec91df0f5c1a5ebe3b8cbfee02"
export const WORLD_ADDRESS = "0x126c4c3e537fb74ced27b005d9db03a086da316a034519df97fe16a817c717f"
export const EVENT_KEY = "0x1a2f334228cee715f1f0f54053bb6b5eac54fa336e0bc1aacf7516decb0471d"


export type SetupNetworkResult = Awaited<ReturnType<typeof setupNetwork>>;

export async function setupNetwork() {

    const contractComponents = defineContractComponents(world);

    const provider = new Providers.RPCProvider(WORLD_ADDRESS);

    const signer = new Account(provider.sequencerProvider, KATANA_ACCOUNT_1_ADDRESS, ec.getKeyPair(KATANA_ACCOUNT_1_PRIVATEKEY))

    const syncWorker = new SyncWorker(provider, contractComponents, EVENT_KEY);

    return {
        contractComponents,
        provider,
        signer,
        execute: async (system: string, call_data: number.BigNumberish[]) => provider.execute(signer, system, call_data),
        entity: async (component: string, query: Query) => provider.entity(component, query),
        entities: async (component: string) => provider.entities(component),
        world,
        syncWorker
    };
}