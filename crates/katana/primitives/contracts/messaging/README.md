# Smart contracts

## Requirements

Please before starting, install:

-   [scarb](https://docs.swmansion.com/scarb/) to build cairo contracts.
-   [starkli](https://github.com/xJonathanLEI/starkli) to interact with Katana.
-   [foundry](https://book.getfoundry.sh/getting-started/installation) to interact with Anvil.

If it's the first time you run the example file, please install forge dependencies:
```bash
cd ~/dojo/crates/katana/core/contracts/messaging/solidity
forge install
```

## Contracts

In this folder you will find smart contracts ready to be declared / deployed
to test how the messaging can work.

## L1 (Ethereum) - L2 (Starknet)

The first messaging is L1-L2 messaging, where Katana is used as a dev Starknet
sequencer. In this scenario, you want to spin up Katana to test your Starknet
contracts before reaching the testnet and use Anvil to dev on Ethereum.

To test this scenario, you can use the associated Makefiles. But the flow is the following:

1. Starting Anvil and deploy the `StarknetMessagingLocal` that simulates the work
   done by the Starknet contract on Ethereum for messaging. Then deploy the `Contract1.sol`
   on ethereum to send/consume messages from Starknet.

2. Starting Katana as your Starknet dev node and declare/deploy `contract_msg_l1.cairo` contract. This contract has example to send/receive messages from Ethereum.

3. Then you can use `starkli` and `cast` to target the contracts on both chain to test
   messaging in both ways.

How to run the scripts:

-   Start Anvil in a terminal.
-   Start Katana in an other terminal on default port 5050 with the messaging configuration that is inside the:
    `katana --messaging ~/dojo/crates/katana/core/contracts/messaging/anvil.messaging.json`
-   Open an other terminal and `cd ~/dojo/crates/katana/core/contracts/messaging`.

Then you can then use pre-defined commands to interact with the contracts.
If you change the code or addresses, you may want to edit the Makefile. But
those Makefiles are only here for quick testing while developing on messaging
and quick demo.

```bash
# Setup anvil with messaging + Contract1.sol deployed.
make -sC ./solidity/ deploy_messaging_contracts

# Declare and deploy contract_msg_l1.cairo.
make -sC ./cairo/ setup_for_l1_messaging

# Send message L1 -> L2 with a single value.
make -sC solidity/ send_msg selector_str=msg_handler_value payload="[123]"

# Send message L1 -> L2 with a serialized struct.
make -sC solidity/ send_msg selector_str=msg_handler_struct payload="[1,2]"

# Send message L2 -> L1 to be manually consumed.
make -sC cairo/ send_msg_value_l1 value=2
```
Then you've to wait the message to be sent to L1, Katana will display it:
```
2023-12-15T15:16:18.435370Z  INFO messaging: Message sent to settlement layer:
|     hash     | 0x62c7475daef517f6858a6f539bb4d2aa7eb1e23a7e8b1bc6a0834256d995e49d
| from_address | 0x4231f608ea4a233136f6cdfcd10eaad2e46362bbc4e5d5aa88d0d574ea120d8
|  to_address  | 0xe7f1725e7734ce288f8367e1bb143e90bb3f0512
|   payload    | [0x2]
```
```
# Consume the messag previously sent. You can try to call it once and see the second one reverting.
make -sC solidity/ consume_msg payload="[2]"
```

## L2 (Starknet) - L3 (Appchain) [Experimental]

The second messaging is when you may want your appchain (Katana based) to communicate
with Starknet. In this case, the Katana sequencer (L3) will listen to the messages
emitted by a specific messaging contract on Starknet.

The messaging in this scenario works exactly the same way as it does for L1-L2. But in this
case, the settlement layer is not Ethereum, but Starknet.

There is a feature that is experimental, which allows the messages to be `executed` instead
of the regular registering/consumption of the message, which is totally manual.

You can also use the Makefile to setup the chains, but the flow is the following:

1. Starting Katana (1) to simulate Starknet network. On this Katana instance, you will
   deploy `appchain_messaging.cairo` which is the analogue contract of `StarknetMessagingLocal` in the L1-L2 messaging. This contract is responsible for sending/registering/executing messages.
   Then you can deploy `contract_1.cairo` to send/consume message and test the execution
   of smart contract function on Starknet from the appchain.

    You can totally deploy `appchain_messaging.cairo` on Starknet to test. **Please be aware
    this contract is not audited for now and only experimental without security considerations yet**.
    Be sure you control who is able to send/execute messages to be safe.

2. Starting Katana (2) to represent your appchain. On this Katana instance, you will deploy
   `contract_msg_starknet.cairo` contract. This contract can send/execute/receive message
   from/to your Katana (1).

3. Then, you can interact with `contract_msg_starknet.cairo` on the appchain (Katana 2) to send/execute messages on Starknet. On Katana (1) which is Starknet, you can interact with `contract_1.cairo` to send/consume and see contract execution.

How to run the scripts:

-   Starts Katana (1) to simulate starknet on a new terminal with default port 5050.
-   Starts Katana (2) for your appchain on a new terminal with port 6060 and the configuration for messaging: `katana --messaging crates/katana/core/contracts/messaging/l3.messaging.json -p 6060`
-   Open an other terminal and `cd ~/dojo/crates/katana/core/contracts/messaging`.

Then you can then use pre-defined commands to interact with the contracts.
If you change the code or addresses, you may want to edit the Makefile. But
those Makefiles are only here for quick testing while developing on messaging
and quick demo.

```bash
# Setup both katana at once with appchain_messaging and contract_1 on katana 1 (starknet),
# and contract_msg_starknet on katana 2 (l3).
make -sC ./cairo/ setup_l2_messaging
make -sC ./cairo/ setup_l3_messaging

# Send a message L3 -> L2 to be manually consumed.
make -sC ./cairo/ send_msg_value_l2 value=3

# Consume the message on L2 (it's a span, so length first).
make -sC ./cairo/ consume_msg_from_l3 payload="1 3"

# Send a message L3 -> L2 to be executed directly on L2.
make -sC ./cairo/ exec_msg_l2 selector_str=set_value value=2

# Verify the execution by getting the value.
make -sC ./cairo/ get_value_l2

# Send a message L2 -> L3.
# Try to change the value to see the transaction error.
make -sC cairo/ send_msg_l3 selector_str=msg_handler_value value=888
```
