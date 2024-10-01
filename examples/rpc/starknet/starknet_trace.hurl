# starknet_traceTransaction
POST http://0.0.0.0:5050
Content-Type: application/json
{
    "jsonrpc": "2.0",
    "method": "starknet_traceTransaction",
    "params": ["0x1337"],
    "id": 1
}

HTTP 200
[Asserts]
jsonpath "$.error.message" equals "Transaction hash not found"
jsonpath "$.error.code" == 29
jsonpath "$.result" not exists

# starknet_traceBlockTransactions
POST http://0.0.0.0:5050
Content-Type: application/json
{
    "jsonrpc": "2.0",
    "method": "starknet_traceBlockTransactions",
    "params": [{ "block_number": 0 }],
    "id": 1
}

HTTP 200
[Asserts]
jsonpath "$.result" exists
jsonpath "$.result" isEmpty
jsonpath "$.error" not exists

# starknet_traceBlockTransactions
POST http://0.0.0.0:5050
Content-Type: application/json
{
    "jsonrpc": "2.0",
    "method": "starknet_traceBlockTransactions",
    "params": [{ "block_number": 677 }],
    "id": 1
}

HTTP 200
[Asserts]
jsonpath "$.error.message" equals "Block not found"
jsonpath "$.error.code" == 24
jsonpath "$.result" not exists
