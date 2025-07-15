#[derive(Drop)]
pub struct GasCounter {
    pub start: u128,
}

#[generate_trait]
pub impl GasCounterImpl of GasCounterTrait {
    #[inline(always)]
    fn start() -> GasCounter {
        let start = core::testing::get_available_gas();
        core::gas::withdraw_gas().unwrap();
        GasCounter { start }
    }

    #[inline(always)]
    fn end(self: GasCounter) -> u128 {
        let end = core::testing::get_available_gas();
        core::gas::withdraw_gas().unwrap();
        let gas_used = self.start - end;
        gas_used
    }

    #[inline(always)]
    fn end_label(self: GasCounter, name: ByteArray) -> u128 {
        let end = core::testing::get_available_gas();
        core::gas::withdraw_gas().unwrap();
        let gas_used = self.start - end;
        println!("#GAS# {}: {}", name, gas_used);
        gas_used
    }

    #[inline(always)]
    fn end_csv(self: GasCounter, name: ByteArray) -> u128 {
        let end = core::testing::get_available_gas();
        core::gas::withdraw_gas().unwrap();
        let gas_used = self.start - end;
        println!("#GAS#{};{}", name, gas_used);
        gas_used
    }
}
