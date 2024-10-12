ACCOUNT_L2=./account_l2.json
ACCOUNT_L2_ADDR=0x127fd5f1fe78a71f8bcd1fec63e3fe2f0486b6ecd5c86a0466c3a21fa5cfcec
L2_PRIVATE_KEY=0x1800000000300000180000000000030000000000003006001800006600

# Build files helpers.
build = ./target/dev/katana_messaging_
sierra = .contract_class.json

#################
# ** L1 <-> L2 **
#
L1_CONTRACT_ADDR=0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512
C_MSG_L1_ADDR=0x0609f8e7a76b6cc36f3ff86f09f6e5fdd0e6320f117d817e4344c1bf9fac7d67

OPTS_L2 := --account katana-0 \
	--rpc http://0.0.0.0:5050 \
	--poll-interval 1000 \
	-w

setup_for_l1_messaging:
	scarb build; \
	class_hash=$$(starkli declare ${build}contract_msg_l1${sierra} ${OPTS_L2}); \
	sleep 1; \
	starkli deploy --salt 0x1234 "$${class_hash}" ${OPTS_L2}

send_msg_value_l1_usage:
	@echo make send_msg_value_l1 value=2

send_msg_value_l1:
	starkli invoke ${C_MSG_L1_ADDR} send_message_value ${L1_CONTRACT_ADDR} $(value) ${OPTS_L2}

send_msg_struct_l1_usage:
	@echo make send_msg_struct_l1 data=\"123 88\"

send_msg_struct_l1:
	starkli invoke ${C_MSG_L1_ADDR} send_message_struct_l1 ${L1_CONTRACT_ADDR} $(data) ${OPTS_L2}


#################
# ** L2 <-> L3 **
#
ACCOUNT_L3=./account_l3.json
ACCOUNT_L3_ADDR=0x127fd5f1fe78a71f8bcd1fec63e3fe2f0486b6ecd5c86a0466c3a21fa5cfcec
L3_PRIVATE_KEY=0x1800000000300000180000000000030000000000003006001800006600

L2_APPCHAIN_MSG_ADDR=0x0368407c4a0e075c693cf372dd735750df7289c6b5d06234615018456e658591
L2_APPCHAIN_MSG_CLASS_HASH=$(shell starkli class-hash target/dev/katana_messaging_appchain_messaging.contract_class.json)

L2_CONTRACT1_ADDR=0x0450ae47f57d4a2165b015a4bf143cd53f60d61a74a0be998bf0a42c477f26ce
L2_CONTRACT1_CLASS_HASH=$(shell starkli class-hash target/dev/katana_messaging_contract_1.contract_class.json)

L3_C_MSG_ADDR=0x039bb4ce38513597cf75eeacd7f3ed6ef058a61818c252612d134d95ed2e9051
L3_C_MSG_CLASS_HASH=$(shell starkli class-hash target/dev/katana_messaging_contract_msg_starknet.contract_class.json)

OPTS_L3 := --account ${ACCOUNT_L3} \
	--rpc http://0.0.0.0:6060 \
	--private-key ${L3_PRIVATE_KEY} \
	--poll-interval 1000 \
	-w

setup_l2_messaging:
	@set -x; \
	scarb build; \
	appchain_ch=$$(starkli class-hash ${build}appchain_messaging${sierra}); \
	starkli declare target/dev/katana_messaging_appchain_messaging.contract_class.json ${OPTS_L2}; \
	starkli declare target/dev/katana_messaging_contract_1.contract_class.json ${OPTS_L2}; \
	starkli deploy --salt 0x1234 ${L2_APPCHAIN_MSG_CLASS_HASH} ${ACCOUNT_L2_ADDR} ${ACCOUNT_L3_ADDR} ${OPTS_L2}; \
	starkli deploy --salt 0x1234 ${L2_CONTRACT1_CLASS_HASH} ${L2_APPCHAIN_MSG_ADDR} ${OPTS_L2}

setup_l3_messaging:
	scarb build
	starkli declare target/dev/katana_messaging_contract_msg_starknet.contract_class.json ${OPTS_L3}
	starkli deploy --salt 0x1234 ${L3_C_MSG_CLASS_HASH} ${OPTS_L3}

send_msg_value_l2_usage:
	@echo make send_msg_value_l2 value=2

send_msg_value_l2:
	starkli invoke ${L3_C_MSG_ADDR} send_message \
	${L2_CONTRACT1_ADDR} \
	$(value) \
	${OPTS_L3}

consume_msg_from_l3_usage:
	@echo make consume_msg_from_l3 payload=\"1 2\"

consume_msg_from_l3:
	starkli invoke ${L2_CONTRACT1_ADDR} consume_message \
	${L3_C_MSG_ADDR} \
	$(payload) \
	${OPTS_L2}

exec_msg_l2_usage:
	@echo make exec_msg_l2 selector_str=set_value value=2

exec_msg_l2:
	$(eval selector=$(shell starkli selector $(selector_str)))
	starkli invoke ${L3_C_MSG_ADDR} execute_message \
	${L2_CONTRACT1_ADDR} \
	${selector} \
	$(value) \
	${OPTS_L3}

get_value_l2:
	starkli call ${L2_CONTRACT1_ADDR} get_value --rpc http://0.0.0.0:5050

send_msg_l3_usage:
	@echo make send_msg_l3 selector_str=msg_handler_value value=2

send_msg_l3:
	$(eval selector=$(shell starkli selector $(selector_str)))
	starkli invoke ${L2_CONTRACT1_ADDR} send_message \
	${L3_C_MSG_ADDR} \
	$(selector) \
	$(value) \
	${OPTS_L2}
