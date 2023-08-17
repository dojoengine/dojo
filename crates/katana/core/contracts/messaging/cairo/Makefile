include .katana.env
export $(shell sed 's/=.*//' .katana.env)

CLASS_HASH := $(shell starkli class-hash target/dev/katana_messaging_contract_1.sierra.json)

OPTS := --account "${STARKNET_ACCOUNT}" --rpc "${STARKNET_RPC}" --private-key "${PRIVATE_KEY}"

declare_deploy:
	$(eval class_hash=$(shell starkli declare target/dev/katana_messaging_contract_1.sierra.json $(OPTS)))
	starkli deploy --salt 0x1234 $(class_hash) $(OPTS)

send_msg_value_usage:
	@echo make send_msg_value l2_contract=0x123 l1_contract=0x456 value=2

send_msg_value:
	starkli invoke $(l2_contract) send_message_value $(l1_contract) $(value) $(OPTS)

send_msg_struct_usage:
	@echo make send_msg_struct l2_contract=0x123 l1_contract=0x456 data="123 88"

send_msg_struct:
	starkli invoke $(l2_contract) send_message_struct $(l1_contract) $(data) $(OPTS)
