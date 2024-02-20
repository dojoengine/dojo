/// The transaction data availability mode.
///
/// Specifies a storage domain in Starknet. Each domain has different gurantees regarding
/// availability
#[repr(u64)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DataAvailabilityMode {
    #[default]
    L1 = 0,
    L2 = 1,
}
