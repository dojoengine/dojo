import { KeyPair } from "starknet";
import { Provider, Account, ec } from "starknet";

export class HotAccount {
    private keypair: KeyPair
    public account: Account
    public address: string

    constructor(provider: Provider, address: string, pk: string) {
        this.address = address
        this.keypair = ec.getKeyPair(pk);
        this.account = new Account(provider, address, this.keypair)
    }
}