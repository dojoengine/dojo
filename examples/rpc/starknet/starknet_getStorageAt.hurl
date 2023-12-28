# Get the value for the ERC20 token name storage slot for the fee token.
POST http://0.0.0.0:5050
Content-Type: application/json
{
    "jsonrpc": "2.0",
    "method": "starknet_getStorageAt",
    "params": [
        "0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7",
		"0x0341c1bdfd89f69748aa00b5742b03adbffd79b8e80cab5c50d91cd8c2a79be1",
		"pending"
    ],
    "id": 1
}

HTTP 200
[Asserts]
jsonpath "$.error" not exists
jsonpath "$.result" matches /^0x[A-Fa-f0-9]+$/


# Uninitialized storage slot of a contract should default to zero.
POST http://0.0.0.0:5050
Content-Type: application/json
{
    "jsonrpc": "2.0",
    "method": "starknet_getStorageAt",
    "params": [
        "0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7",
		"0x3434343434",
		"pending"
    ],
    "id": 1
}

HTTP 200
[Asserts]
jsonpath "$.error" not exists
jsonpath "$.result" equals "0x0"


# Fetching storage slot of non-existant contract should fail.
POST http://0.0.0.0:5050
Content-Type: application/json
{
    "jsonrpc": "2.0",
    "method": "starknet_getStorageAt",
    "params": [
        "0xdead",
		"0xdead",
		"pending"
    ],
    "id": 1
}

HTTP 200
[Asserts]
jsonpath "$.error.message" equals "Contract not found"
jsonpath "$.result" not exists
