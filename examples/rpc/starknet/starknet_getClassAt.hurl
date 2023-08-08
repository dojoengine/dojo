POST http://0.0.0.0:5050
Content-Type: application/json
{
    "jsonrpc": "2.0",
    "method": "starknet_getClassAt",
    "params": [
        "latest",
        "0x3ee9e18edc71a6df30ac3aca2e0b02a198fbce19b7480a63a0d71cbd76652e0"
    ],
    "id":1
}

HTTP 200
[Asserts]
jsonpath "$.error" not exists
jsonpath "$.result" exists
