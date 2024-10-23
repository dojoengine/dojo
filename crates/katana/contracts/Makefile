CONTRACT_CLASS_SUFFIX := .contract_class.json
COMPILED_CLASS_SUFFIX := .compiled_contract_class.json
# Directory where the compiled classes will be stored
BUILD_DIR := ./build

## ---- Default Pre-deployed Account

ORIGINAL_CLASS_NAME := katana_account_Account$(CONTRACT_CLASS_SUFFIX)
CLASS_NAME := default_account.json

$(BUILD_DIR): ./account/src/*
	scarb build -p katana_account
	mv target/dev/$(ORIGINAL_CLASS_NAME) $(BUILD_DIR)/$(CLASS_NAME)

## ---- Test Contracts

# I only care about the casm class for this one, hence this
ORIGINAL_CLASS_NAME := katana_test_contracts_GetChainId$(CONTRACT_CLASS_SUFFIX)
CLASS_NAME := test_get_chain_id.json

$(BUILD_DIR): ./test-contracts/src/*
	scarb build -p katana_test_contracts
	mv target/dev/$(ORIGINAL_CLASS_NAME) $(BUILD_DIR)/$(CLASS_NAME)
