# `scheduler`

```sh
cargo run --bin scheduler -- --world 42 your_input.json your_input2.json ... your_input2^n
```
## number of inputs have to be power of 2
# input format example

```json
{
    "prev_state_root":101, 
    "block_number":102, 
    "block_hash":103, 
    "config_hash":104, 
    "message_to_starknet_segment":[105,106,1,1], 
    "message_to_appchain_segment":[108,109,110,111,1,112],
    "nonce_updates":{},
    "storage_updates":{
        "42": {
            "2010": 1200,
            "2012": 1300
        }
    },
    "contract_updates":{},
    "declared_classes":{}
}
```

# output
## scheduler outputs map of proofs in result.json file 
```json
{
    "proof1": proof,
    "proof2": proof,
}
```