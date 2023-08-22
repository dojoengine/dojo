// ERC165 
const IERC165_ID: u32 = 0x01ffc9a7_u32;
const IACCOUNT_ID: u32 = 0xa66bd575_u32;

#[starknet::interface]
trait IERC165<TState> {
    fn supports_interface(self: @TState, interface_id: u32) -> bool;
}
