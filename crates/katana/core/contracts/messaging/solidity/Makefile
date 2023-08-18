# Only .env file is loaded by foundry, and we can't specify a file.
# Do not forget to copy your config into `.env`.
include .env
export $(shell sed 's/=.*//' .env)

deploy:
	forge script --broadcast --rpc-url ${ETH_RPC_URL} script/LocalTesting.s.sol:LocalSetup

send_msg_usage:
	@echo make send_msg l1_contract=0x123 l2_contract=0x456 selector_str=funct_name payload=\"[1,2]\"

send_msg:
	$(eval selector=$(shell starkli selector $(selector_str)))
	cast send $(l1_contract) \
	"sendMessage(uint256,uint256,uint256[])" \
	$(l2_contract) ${selector} $(payload) \
	--private-key ${ACCOUNT_PRIVATE_KEY} --value 1

consume_msg_usage:
	@echo make consume_msg l1_contract=0x123 l2_account=0x823 payload=\"[1,2]\"

consume_msg:
	cast send $(l1_contract) \
	"consumeMessage(uint256,uint256[])" \
	$(l2_account) $(payload) \
	--private-key ${ACCOUNT_PRIVATE_KEY}
