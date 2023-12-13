POST http://0.0.0.0:5050
Content-Type: application/json
{
    "jsonrpc": "2.0",
    "method": "starknet_getBlockWithTxHashes",
    "params": [
        {
            "block_number": 0
        }
    ],
    "id":1
}

HTTP 200
[Asserts]
jsonpath "$.error" not exists
jsonpath "$.result.transactions" isCollection
jsonpath "$.result.block_number" isInteger
jsonpath "$.result.block_hash" matches /^0x[A-Fa-f0-9]+$/
jsonpath "$.result.parent_hash" matches /^0x[A-Fa-f0-9]+$/
jsonpath "$.result.starknet_version" matches /^[0-9]+.[0-9]+.[0-9]+$/

