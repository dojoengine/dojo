import { defineContractComponents } from "./contractComponents";
import { world } from "./world";
import { number } from 'starknet';

import Manifest from './manifest.json'

import { Providers, Query } from "@dojoengine/core";
import { Account, ec } from "starknet";

export const KATANA_ACCOUNT_1_ADDRESS = "0x06f62894bfd81d2e396ce266b2ad0f21e0668d604e5bb1077337b6d570a54aea"
export const KATANA_ACCOUNT_1_PRIVATEKEY = "0x07230b49615d175307d580c33d6fda61fc7b9aec91df0f5c1a5ebe3b8cbfee02"


export type SetupNetworkResult = Awaited<ReturnType<typeof setupNetwork>>;

export async function setupNetwork() {

    const contractComponents = defineContractComponents(world);

    const provider = new Providers.RPCProvider(Manifest.world);

    const signer = new Account(provider.sequencerProvider, KATANA_ACCOUNT_1_ADDRESS, ec.getKeyPair(KATANA_ACCOUNT_1_PRIVATEKEY))

    return {
        contractComponents,
        provider,
        signer,
        execute: async (system: string, call_data: number.BigNumberish[]) => provider.execute(signer, system, call_data),
        entity: async (component: string, query: Query) => provider.entity(component, query),
        entities: async (component: string, partition: string) => provider.entities(component, partition),
        world
    };
}