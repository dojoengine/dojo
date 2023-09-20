# Setups contracts on existing devnodes to dev quicker.

# running in an other terminal for debugging.
run_dev_eth:
	cargo build -p katana

	@make run_anvil

	sleep 1

	make -C ../../contracts/messaging/solidity/ deploy

	cargo run -p katana -- \
	--messaging ../../contracts/messaging/messaging.json

run_anvil:
	- pkill anvil
	anvil &
