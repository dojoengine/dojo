
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

# Now message sending is asynchronous, so the message must first be settled
# before being consumed.
sleep 20

# Consume the message previously sent. You can try to call it once and see the second one reverting.
make -sC solidity/ consume_msg payload="[2]"
