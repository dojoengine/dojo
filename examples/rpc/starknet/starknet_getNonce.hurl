POST http://0.0.0.0:5050
Content-Type: application/json
{
    "jsonrpc": "2.0",
    "method": "starknet_getNonce",
    "params": [
        "latest",
        "0x05b92948371346d0df1d3c2d7568573364497f6cba65f4734ecd54ed0a0dbd11"
    ],
    "id":1
}

HTTP 200
[Asserts]
jsonpath "$.error" not exists
jsonpath "$.result" matches /^0x[A-Fa-f0-9]+$/
