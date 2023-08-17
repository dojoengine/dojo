include .katana.env
export $(shell sed 's/=.*//' .katana.env)

CLASS_HASH := $(shell starkli class-hash target/dev/katana_messaging_contract_1.sierra.json)

OPTS := --account "${STARKNET_ACCOUNT}" --rpc "${STARKNET_RPC}" --private-key "${PRIVATE_KEY}"

declare_deploy:
	$(eval class_hash=$(shell starkli declare target/dev/katana_messaging_contract_1.sierra.json $(OPTS)))
	starkli deploy $(class_hash) $(OPTS)

send_msg_usage:
	@echo make send_msg l2_contract=0x123 l1_contract=0x456 payload="2 1 2"

send_msg:
	starkli invoke $(l2_contract) send_message $(l1_contract) $(payload) $(OPTS)
