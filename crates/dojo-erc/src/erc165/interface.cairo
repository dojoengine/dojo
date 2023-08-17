#[starknet::interface]
trait IERC165<TState> {
    fn supports_interface(self: TState, interface_id: u32) -> bool;
}
