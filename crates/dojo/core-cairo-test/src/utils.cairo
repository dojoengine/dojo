
#[derive(Drop)]
pub struct GasCounter {
    pub start: u128,
}

#[generate_trait]
pub impl GasCounterImpl of GasCounterTrait {
    fn start() -> GasCounter {
        let start = core::testing::get_available_gas();
        core::gas::withdraw_gas().unwrap();
        GasCounter { start }
    }

    fn end(self: GasCounter, name: ByteArray) {
        let end = core::testing::get_available_gas();
        let gas_used = self.start - end;

        println!("# GAS # {}: {}", Self::pad_start(name, 18), gas_used);
        core::gas::withdraw_gas().unwrap();
    }

    fn pad_start(str: ByteArray, len: u32) -> ByteArray {
        let mut missing: ByteArray = "";
        let missing_len = if str.len() >= len {
            0
        } else {
            len - str.len()
        };

        while missing.len() < missing_len {
            missing.append(@".");
        };
        missing + str
    }
}

// assert that `value` and `expected` have the same size and the same content
pub fn assert_array(value: Span<felt252>, expected: Span<felt252>) {
    assert!(value.len() == expected.len(), "Bad array length");

    let mut i = 0;
    loop {
        if i >= value.len() {
            break;
        }

        assert!(
            *value.at(i) == *expected.at(i),
            "Bad array value [{}] (expected: {} got: {})",
            i,
            *expected.at(i),
            *value.at(i)
        );

        i += 1;
    }
}
