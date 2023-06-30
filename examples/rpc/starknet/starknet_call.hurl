POST http://0.0.0.0:5050
Content-Type: application/json
{
    "jsonrpc":"2.0",
    "method":"starknet_call",
    "params": [
        {
			"contract_address": "0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7",
			"entry_point_selector": "0x2e4263afad30923c891518314c3c95dbe830a16874e8abc5777a9a20b54c76e",
			"calldata": [
				"0x05b92948371346d0df1d3c2d7568573364497f6cba65f4734ecd54ed0a0dbd11"
			]
		},
		"pending"
    ],
    "id":1
}

HTTP 200
[Asserts]
jsonpath "$.error" not exists
jsonpath "$.result" exists
