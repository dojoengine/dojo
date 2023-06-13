// import worldsJson from "contracts/worlds.json";

const worlds = worldsJson as Partial<Record<string, { address: string; blockNumber?: number }>>;

type NetworkConfig = SetupContractConfig & {
    privateKey: string;
    faucetServiceUrl?: string;
    snapSync?: boolean;
};

// providers

export async function getNetworkConfig(): Promise<NetworkConfig> {

    return {
        provider: {
            rpc: ""
        },
        privateKey: getBurnerWallet().value
    };
}