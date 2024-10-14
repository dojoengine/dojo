POST http://0.0.0.0:5050
Content-Type: application/json
{
    "jsonrpc": "2.0",
    "method": "starknet_getClassAt",
    "params": [
        "latest",
        "0x2af9427c5a277474c079a1283c880ee8a6f0f8fbf73ce969c08d88befec1bba"
    ],
    "id":1
}

HTTP 200
[Asserts]
jsonpath "$.error" not exists
jsonpath "$.result" exists
