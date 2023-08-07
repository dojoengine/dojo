![katana](../../.github/katana-mark.svg)

`katana` is a _blazingly fast_ local Starknet node, designed to support local development with Dojo.

## Features

-   [Starknet JSON-RPC v0.3.0](https://github.com/starkware-libs/starknet-specs/tree/v0.3.0) support

## Installation

`katana` binary is available via [`dojoup`](../../README.md#installation).

### Installing from source

```sh
git clone https://github.com/dojoengine/dojo
cd dojo
cargo install --path ./crates/katana --locked --force
```

## StarkNet Features Compatibility

### Transaction

| Feature        | State              | Version |
| -------------- | ------------------ | ------- |
| invoke         | :white_check_mark: | 1       |
| declare        | :white_check_mark: | 1, 2    |
| deploy_account | :white_check_mark: |         |

### Supported RPC

| Feature                                  | State              |
| ---------------------------------------- | ------------------ |
| **Read**                                 |
| starknet_getBlockWithTxHashes            | :white_check_mark: |
| starknet_getBlockWithTxs                 | :white_check_mark: |
| starknet_getStateUpdate                  | :white_check_mark: |
| starknet_getStorageAt                    | :white_check_mark: |
| starknet_getTransactionByHash            | :white_check_mark: |
| starknet_getTransactionByBlockIdAndIndex | :white_check_mark: |
| starknet_getTransactionReceipt           | :white_check_mark: |
| starknet_getClass                        | :white_check_mark: |
| starknet_getClassHashAt                  | :white_check_mark: |
| starknet_getClassAt                      | :white_check_mark: |
| starknet_getBlockTransactionCount        | :white_check_mark: |
| starknet_call                            | :white_check_mark: |
| starknet_estimateFee                     | :white_check_mark: |
| starknet_blockNumber                     | :white_check_mark: |
| starknet_blockHashAndNumber              | :white_check_mark: |
| starknet_chainId                         | :white_check_mark: |
| starknet_pendingTransactions             | :white_check_mark: |
| starknet_syncing                         | :construction:     |
| starknet_getEvents                       | :white_check_mark: |
| starknet_getNonce                        | :white_check_mark: |
| **Trace**                                |
| starknet_traceTransaction                | :construction:     |
| starknet_simulateTransaction             | :construction:     |
| starknet_traceBlockTransactions          | :construction:     |
| **Write**                                |
| starknet_addInvokeTransaction            | :white_check_mark: |
| starknet_addDeclareTransaction           | :white_check_mark: |
| starknet_addDeployAccountTransaction     | :white_check_mark: |

## Getting started

```console



██╗  ██╗ █████╗ ████████╗ █████╗ ███╗   ██╗ █████╗
██║ ██╔╝██╔══██╗╚══██╔══╝██╔══██╗████╗  ██║██╔══██╗
█████╔╝ ███████║   ██║   ███████║██╔██╗ ██║███████║
██╔═██╗ ██╔══██║   ██║   ██╔══██║██║╚██╗██║██╔══██║
██║  ██╗██║  ██║   ██║   ██║  ██║██║ ╚████║██║  ██║
╚═╝  ╚═╝╚═╝  ╚═╝   ╚═╝   ╚═╝  ╚═╝╚═╝  ╚═══╝╚═╝  ╚═╝



PREFUNDED ACCOUNTS
==================

| Account address |  0x03ee9e18edc71a6df30ac3aca2e0b02a198fbce19b7480a63a0d71cbd76652e0
| Private key     |  0x0300001800000000300000180000000000030000000000003006001800006600
| Public key      |  0x01b7b37a580d91bc3ad4f9933ed61f3a395e0e51c9dd5553323b8ca3942bb44e

| Account address |  0x033c627a3e5213790e246a917770ce23d7e562baa5b4d2917c23b1be6d91961c
| Private key     |  0x0333803103001800039980190300d206608b0070db0012135bd1fb5f6282170b
| Public key      |  0x04486e2308ef3513531042acb8ead377b887af16bd4cdd8149812dfef1ba924d

| Account address |  0x01d98d835e43b032254ffbef0f150c5606fa9c5c9310b1fae370ab956a7919f5
| Private key     |  0x07ca856005bee0329def368d34a6711b2d95b09ef9740ebf2c7c7e3b16c1ca9c
| Public key      |  0x07006c42b1cfc8bd45710646a0bb3534b182e83c313c7bc88ecf33b53ba4bcbc

| Account address |  0x0697aaeb6fb12665ced647f7efa57c8f466dc3048556dd265e4774c546caa059
| Private key     |  0x009f6d7a28c0aec0bb42b11600b2fdc4f20042ab6adeac0ca9e6696aabc5bc95
| Public key      |  0x076e247c83b961e3ac33082406498a8629a51c1c9e465f4302018565ec1841ff

| Account address |  0x021b8eb1d455d5a1ef836a8dae16bfa61fbf7aaa252384ab4732603d12d684d2
| Private key     |  0x05d4184feb2ba1aa1274885dd88c8a670a806066dda7684aa562390441224483
| Public key      |  0x04e8b088e35962a3912054065682b0546921a96f0b63418484c824ed67729ba3

| Account address |  0x018e623c4ee9f3cf93b06784606f5bc1e86070e8ee6459308c9482554e265367
| Private key     |  0x01c62fa406d5cac0f365e20ae9c365548f793196e40536c8c118130255a0ac54
| Public key      |  0x04796fb56fc9e44bf543c56625527c04e0a6c51b76b00fc95d8b18749f051077

| Account address |  0x01a0a8e7c3a71a44d2e43fec473d7517fd4f20c6ea054e33be3f98ef82e449df
| Private key     |  0x07813a0576f69d6e2e90d6d5d861f029fa34e528ba418ebb8e335dbc1ed18505
| Public key      |  0x04fa2c2e826b04cdf46625c4e80a6e24c8c1e629ccedcbed7dcbe2cf2dd0a6da

| Account address |  0x006a933941976911cbf6917010aae47ef7a54bb32846a3d890c1985d879807aa
| Private key     |  0x0092f44f50c2fe38cdd00c59a8ab796238982426341f0ee9ebcaa7fd8b1ac939
| Public key      |  0x03aa57dec32a26b97ab2542da41ca2512cfc5e3ffc3feca2d21664e2eeeb3836

| Account address |  0x03c00d7cda80f89cb59c147d897acb1647f9e33228579674afeea08f6f57e418
| Private key     |  0x04f5adc57e9025a7c5d1424972354fd83ace8b60ff7d46251512b3ea69b81434
| Public key      |  0x03839dd0e4e8e664f659b580a9d04de2984914a86e055f46ad2abd687bf4225d

| Account address |  0x04514dd4ce4762369fc108297f45771f5160aeb7c864d5209e5047a48ab90b52
| Private key     |  0x04929b5202c17d1bf1329e0f3b1deac313252a007cfd925d703e716f790c5726
| Public key      |  0x0250a4e65d6d55cbb2a643585a92891b3950c841f30c79ab8b3ee5ee2c3f4194


ACCOUNTS SEED
=============
0


🚀 JSON-RPC server started: http://0.0.0.0:5050


```

Check out the [`katana Reference`](https://book.dojoengine.org/reference/katana/index.html) section of the Dojo book to learn about all the available RPC methods and options.
