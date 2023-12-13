POST http://0.0.0.0:5050
Content-Type: application/json
{
    "jsonrpc": "2.0",
    "method": "starknet_getTransactionStatus",
    "params": [
        "0x1"
    ],
    "id": 1
}

HTTP 200
[Asserts]
jsonpath "$.error.message" equals "Transaction hash not found"
jsonpath "$.result" not exists
