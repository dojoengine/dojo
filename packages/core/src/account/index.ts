import { KeyPair } from "starknet";
import { Provider, Account, ec } from "starknet";
import { KATANA_ACCOUNT_1_ADDRESS, KATANA_ACCOUNT_1_PRIVATEKEY } from '../constants';

export class HotAccount {
    private keypair: KeyPair
    public account: Account
    public address: string

    constructor(provider: Provider, address: string = KATANA_ACCOUNT_1_ADDRESS, pk: string = KATANA_ACCOUNT_1_PRIVATEKEY) {
        this.address = address
        this.keypair = ec.getKeyPair(pk);
        this.account = new Account(provider, address, this.keypair)
    }
}