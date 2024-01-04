ACCOUNT_L2=./account_l2.json
ACCOUNT_L2_ADDR=0x9238c8ca6b3c6ab45a793593b13d98797ccd3bda179d313553e51fee114624
L2_PRIVATE_KEY=0x1800000000300000180000000000030000000000003006001800006600

# Build files helpers.
build = ./target/dev/katana_messaging_
sierra = .contract_class.json

#################
# ** L1 <-> L2 **
#
L1_CONTRACT_ADDR=0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512
C_MSG_L1_ADDR=0x04231f608ea4a233136f6cdfcd10eaad2e46362bbc4e5d5aa88d0d574ea120d8

OPTS_L2 := --account katana-0 \
	--rpc http://0.0.0.0:5050

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
ACCOUNT_L3_ADDR=0x5686a647a9cdd63ade617e0baf3b364856b813b508f03903eb58a7e622d5855
L3_PRIVATE_KEY=0x33003003001800009900180300d206308b0070db00121318d17b5e6262150b

# L2_APPCHAIN_MSG_ADDR=0x046c0ea3fb2ad27053e8af3c8cfab38a51afb9fe90fcab1f75446bd41f7d3796
# L2_APPCHAIN_MSG_CLASS_HASH=$(shell starkli class-hash target/dev/katana_messaging_appchain_messaging.contract_class.json)

L2_CONTRACT1_ADDR=0x054f66c104745e27ad5194815a6c4755cf2076c4809212101dfe31563f312a34
L2_CONTRACT1_CLASS_HASH=$(shell starkli class-hash target/dev/katana_messaging_contract_1.contract_class.json)

L3_C_MSG_ADDR=0x071278839029ab1f9fa0ce1ee01e38599736dd4e8fed2417158bec4ef5dc6d0f
L3_C_MSG_CLASS_HASH=$(shell starkli class-hash target/dev/katana_messaging_contract_msg_starknet.contract_class.json)

OPTS_L3 := --account ${ACCOUNT_L3} \
	--rpc http://0.0.0.0:6060 \
	--private-key ${L3_PRIVATE_KEY}

setup_l2_messaging:
	@set -x; \
	scarb build; \
	appchain_ch=$$(starkli class-hash ${build}ERC20${sierra}); \
	starkli declare target/dev/katana_messaging_appchain_messaging.contract_class.json ${OPTS_L2}
	starkli declare target/dev/katana_messaging_contract_1.contract_class.json ${OPTS_L2}
	starkli deploy --salt 0x1234 ${L2_APPCHAIN_MSG_CLASS_HASH} ${ACCOUNT_L2_ADDR} ${ACCOUNT_L3_ADDR} ${OPTS_L2}
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
	${ACCOUNT_L3_ADDR} \
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
