use roaring::RoaringTreemap;
use serde::{Deserialize, Serialize};

/// Stores a list of block numbers.
/// Mainly used for changeset tables to store the list of block numbers where a change occurred.
pub type BlockList = IntegerSet;

/// A set for storing integer values.
///
/// The list is stored in a Roaring bitmap data structure as it uses less space compared to a normal
/// bitmap or even a naive array with similar cardinality.
///
/// See <https://www.roaringbitmap.org/>.
#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct IntegerSet(RoaringTreemap);

impl IntegerSet {
    pub fn new() -> Self {
        Self(RoaringTreemap::new())
    }

    /// Insert a new number to the set.
    pub fn insert(&mut self, num: u64) {
        self.0.insert(num);
    }

    /// Checks if the set contains the given number.
    pub fn contains(&self, num: u64) -> bool {
        self.0.contains(num)
    }

    /// Returns the number of elements in the set that are smaller or equal to the given `value`.
    pub fn rank(&self, value: u64) -> u64 {
        self.0.rank(value)
    }

    /// Returns the `n`th integer in the set or `None` if `n >= len()`.
    pub fn select(&self, n: u64) -> Option<u64> {
        self.0.select(n)
    }
}

impl<const N: usize> From<[u64; N]> for IntegerSet {
    fn from(arr: [u64; N]) -> Self {
        Self(RoaringTreemap::from_iter(arr))
    }
}
