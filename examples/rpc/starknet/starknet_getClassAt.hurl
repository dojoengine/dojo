POST http://0.0.0.0:5050
Content-Type: application/json
{
    "jsonrpc": "2.0",
    "method": "starknet_getClassAt",
    "params": [
        "latest",
        "0x9238c8ca6b3c6ab45a793593b13d98797ccd3bda179d313553e51fee114624"
    ],
    "id":1
}

HTTP 200
[Asserts]
jsonpath "$.error" not exists
jsonpath "$.result" exists
