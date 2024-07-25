# starknet_traceTransaction
POST http://0.0.0.0:5050
Content-Type: application/json
{
    "jsonrpc": "2.0",
    "method": "starknet_traceTransaction",
    "params": ["0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"],
    "id": 2
}

HTTP 200
[Asserts]
jsonpath "$.error" exists
jsonpath "$.error.code" == -32601
jsonpath "$.error.message" == "Unsupported method - starknet_traceTransaction"
jsonpath "$.result" not exists

# starknet_traceBlockTransactions
POST http://0.0.0.0:5050
Content-Type: application/json
{
    "jsonrpc": "2.0",
    "method": "starknet_traceBlockTransactions",
    "params": ["latest"],
    "id": 1
}

HTTP 200
[Asserts]
jsonpath "$.error" exists
jsonpath "$.error.code" == -32601
jsonpath "$.error.message" == "Unsupported method - starknet_traceBlockTransactions"
jsonpath "$.result" not exists
