use roaring::RoaringTreemap;
use serde::{Deserialize, Serialize};

/// Stores a list of block numbers.
/// Mainly used for changeset tables to store the list of block numbers where a change occurred.
pub type BlockList = IntegerList;

/// A list of integers.
///
/// The list is stored in a Roaring bitmap data structure as it uses less space compared to a normal
/// bitmap or even a naive array with similar cardinality.
///
/// See <https://www.roaringbitmap.org/>.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct IntegerList(RoaringTreemap);

impl IntegerList {
    pub fn new() -> Self {
        Self(RoaringTreemap::new())
    }

    /// Insert a new number to the list.
    pub fn insert(&mut self, num: u64) {
        self.0.insert(num);
    }

    /// Checks if the list contains the given number.
    pub fn contains(&self, num: u64) -> bool {
        self.0.contains(num)
    }

    /// Returns the number of elements in the list that are smaller or equal to the given `value`.
    pub fn rank(&self, value: u64) -> u64 {
        self.0.rank(value)
    }

    /// Returns the `n`th integer in the set or `None` if `n >= len()`.
    pub fn select(&self, n: u64) -> Option<u64> {
        self.0.select(n)
    }
}
