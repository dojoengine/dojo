POST http://0.0.0.0:5050
Content-Type: application/json
{
    "jsonrpc": "2.0",
    "method": "starknet_estimateFee",
    "params": [
		{
			"type": "INVOKE",
			"nonce": "0x0",
			"version": "0x1",
			"max_fee": "0x0",
			"signature": [
			
			],
			"sender_address": "0x049c7a9c0d35fc7af43743e288974b99bb5393496282ec560180ab77120f44a2",
			"calldata": [
				"0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7",
				"0x83afd3f4caedc6eebf44246fe54e38c95e3179a5ec9ea81740eca5b482d12e",
				"0x3",
				"0x03819aca4f147e3b589807dd81257c02c4d616328f8b6bdc097b4ae517130a97",
				"0x112391231998",
				"0x0"
			]
		},
		"pending"
				
		
	],
    "id": 1
}

HTTP 200
[Asserts]
jsonpath "$.error" not exists
jsonpath "$.result" exists
