POST http://0.0.0.0:5050
Content-Type: application/json
{
    "jsonrpc": "2.0",
    "method": "starknet_getClass",
    "params": [
        "latest",
        "0x016c6081eb34ad1e0c5513234ed0c025b3c7f305902d291bad534cd6474c85bc"
    ],
    "id":1
}

HTTP 200
[Asserts]
jsonpath "$.error" not exists
jsonpath "$.result" exists
