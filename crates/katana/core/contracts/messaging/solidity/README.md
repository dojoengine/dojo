## Foundry

Please install [foundry](https://github.com/foundry-rs/foundry) before starting.

To deploy contracts please consider the following:

0. Run anvil: `anvil`.
1. Copy `.anvil.env` into `.env`.
2. `source .env`
3. `forge script --broadcast --rpc-url ${ETH_RPC_URL} script/LocalTesting.s.sol:LocalSetup`

You should now have a json file into `logs` folder with the deployed addresses.

To interact with the node, you can use the Makefile for better UX.
If you need more customization, please check the Makefile to see the commands.

If a command in the makefile requires argument, please use the associated `*_usage`.

If you want to check the logs emitted by the contracts, run `cast logs`.

Note, starknet core contract is expected at least 30k wei to work. So you must
always send a value when calling a function that will send a message to L2.

If the message is not ready yet to be consumed, you should see an error like this
using:

```
(code: 3, message: execution reverted: INVALID_MESSAGE_TO_CONSUME, data: Some(String("0x08c379a00000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000001a494e56414c49445f4d4553534147455f544f5f434f4e53554d45000000000000")))
```

