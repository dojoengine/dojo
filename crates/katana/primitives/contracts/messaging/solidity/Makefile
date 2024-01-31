# Only .env file is loaded by foundry, and we can't specify a file.
# Do not forget to copy your config into `.env`.

# For dev, we always take anvil config.
COPY_CONFIG:=$(shell cp .anvil.env .env)

include .env
export $(shell sed 's/=.*//' .env)

# Addresses fixed here for easy testing.
C_MSG_L2_ADDR=0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512
L2_ACCOUNT=0x6162896d1d7ab204c7ccac6dd5f8e9e7c25ecd5ae4fcb4ad32e57786bb46e03
L2_CONTRACT_ADDR=0x0429a64d97c1422a37a09fc7406f35c264be59b744aaff5a79d59393eb1bc7e1

deploy_messaging_contracts:
	forge script --broadcast --rpc-url ${ETH_RPC_URL} script/LocalTesting.s.sol:LocalSetup

send_msg_usage:
	@echo make send_msg selector_str=func_name payload=\"[1,2]\"

send_msg:
	$(eval selector=$(shell starkli selector $(selector_str)))
	cast send ${C_MSG_L2_ADDR} \
	"sendMessage(uint256,uint256,uint256[])" \
	${L2_CONTRACT_ADDR} ${selector} $(payload) \
	--private-key ${ACCOUNT_PRIVATE_KEY} --value 1

consume_msg_usage:
	@echo make consume_msg payload=\"[1,2]\"

consume_msg:
	cast send ${C_MSG_L2_ADDR} \
	"consumeMessage(uint256,uint256[])" \
	${L2_CONTRACT_ADDR} $(payload) \
	--private-key ${ACCOUNT_PRIVATE_KEY}
