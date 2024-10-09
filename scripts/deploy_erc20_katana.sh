#!/bin/bash

starkli deploy --account ../account.json --keystore ../signer.json --keystore-password "" 0x02a8846878b6ad1f54f6ba46f5f40e11cee755c677f130b2c4b60566c9003f1f 0x626c6f62 0x424c42 0x8 u256:10000000000 0xb3ff441a68610b30fd5e2abbf3a1548eb6ba6f3559f2862bf2dc757e5828ca --rpc http://localhost:5050
