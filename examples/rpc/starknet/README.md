### Testing RPC Endpoints

To test the RPC endpoints, follow the steps below:

Run Katana locally (by default, it runs on port 5050):

```bash
cargo run --bin katana
```

Execute hurl tests sequentially:

```bash
hurl --test examples/rpc/**/*.hurl
```
