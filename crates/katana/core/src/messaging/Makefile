# Setups contracts on existing devnodes to dev quicker.

# running in an other terminal for debugging.
run_dev_eth:
	cargo build -p katana

	@make run_anvil

	sleep 1

	make -C ../../contracts/messaging/solidity/ deploy

	cargo run -p katana -- \
	--messaging-rpc-url http://127.0.0.1:8545 \
	--messaging-private-key ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
	--messaging-contract-address 0x5FbDB2315678afecb367f032d93F642f64180aa3 \
	--messaging-sender-address 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 \
	--messaging-fetch-interval 3

run_anvil:
	- pkill anvil
	anvil &
