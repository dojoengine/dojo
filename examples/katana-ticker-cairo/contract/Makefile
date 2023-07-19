.PHONY: contract clean

CONTRACT=src/counter_target.cairo
CONTRACT_SIERRA=target/dev/counter_target_CounterTarget.sierra.json

contract: ${CONTRACT_SIERRA}

${CONTRACT_SIERRA}: ${CONTRACT}
	scarb build

clean:
	-rm -rf ./target
