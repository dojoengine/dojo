//! Cairo 2.4.0 feature testing.
#[starknet::contract]
mod cairo_v240 {
    use core::fmt::Formatter;

    #[storage]
    struct Storage {}

    fn byte_array(self: @ContractState) -> ByteArray {
        let mut ba: ByteArray = "";
        ba.append_word('ABCDEFGHIJKLMNOPQRSTUVWXYZ12345', 31);
        ba.append_byte(0x65);

        let mut bc: ByteArray = "";
        bc.append(@ba);

        bc
    }

    fn formatter(self: @ContractState) {
        let var = 5;
        let mut formatter: Formatter = Default::default();
        write!(formatter, "test").unwrap();
        write!(formatter, "{var:?}").unwrap();
        println!("{}", formatter.buffer); //prints test5
    }

    fn format(self: @ContractState) {
        let var1 = 5;
        let var2: ByteArray = "hello";
        let var3 = 5_u32;
        let _ba = format!("{},{},{}", var1, var2, var3);
        let _ba = format!("{var1}{var2}{var3}");
        let _ba = format!("{var1:?}{var2:?}{var3:?}");
    }

    fn long_panic(self: @ContractState) {
        panic!("this should not be reached, but at least I'm not limited to 31 characters anymore")
    }

    #[derive(Drop, Debug, PartialEq)]
    struct MyStruct {
        a: u8,
        b: u8
    }

    fn asserts(self: @ContractState) {
        let var1 = 5;
        let var2 = 6;
        assert!(var1 != var2, "should not be equal");
        assert!(var1 != var2, "{},{} should not be equal", var1, var2);

        let first = MyStruct { a: 1, b: 2 };
        let second = MyStruct { a: 1, b: 2 };
        assert!(first == second, "should be equal");
    }
}
