POST http://0.0.0.0:5050
Content-Type: application/json
{
    "jsonrpc": "2.0",
    "method": "starknet_getClassAt",
    "params": [
        "latest",
        "0x5a37d83d451063858217e9c510d6f45d6bd37ff8664a7c0466329316f7a2891"
    ],
    "id":1
}

HTTP 200
[Asserts]
jsonpath "$.error" not exists
jsonpath "$.result" exists
