use katana_primitives::contract::ContractAddress;
use katana_primitives::FieldElement;

/// Trait for writing compact representation of the types that implement it.
pub trait Compact: Sized {
    /// Write the compact representation of `self` into `buf`, returning the number of bytes
    /// written.
    fn to_compact<B>(self, buf: &mut B) -> usize
    where
        B: bytes::BufMut + AsMut<[u8]>;

    /// Read the compact representation of `self` from `buf`, returning the value and the
    /// remaining bytes in the buffer.
    fn from_compact(buf: &[u8], len: usize) -> (Self, &[u8]);
}

macro_rules! impl_compact_for_uints {
    ($($ty:ty),*) => {
        $(
            impl Compact for $ty {
                fn to_compact<B>(self, buf: &mut B) -> usize
                where
                    B: bytes::BufMut + AsMut<[u8]>,
                {
                    // get the leading zeros in term of bytes
                    let zeros = self.leading_zeros() as usize / 8;
                    buf.put_slice(&self.to_be_bytes()[zeros..]);
                    std::mem::size_of::<$ty>() - zeros
                }

                fn from_compact(buf: &[u8], len: usize) -> (Self, &[u8]) {
                    if len > 0 {
                        let mut arr = [0; std::mem::size_of::<$ty>()];
                        arr[std::mem::size_of::<$ty>() - len..].copy_from_slice(&buf[..len]);
                        return (<$ty>::from_be_bytes(arr), &buf[len..])
                    }
                    (0, buf)
                }
            }
        )*
    };
}

macro_rules! impl_compact_felt {
    ($($ty:ty),*) => {
        $(
            impl Compact for $ty {
                fn to_compact<B>(self, buf: &mut B) -> usize
                where
                    B: bytes::BufMut + AsMut<[u8]>,
                {
                    let zeros = self
                        .to_bits_le()
                        .iter()
                        .rev()
                        .position(|n| *n != false)
                        .map_or(32, |pos| pos / 8 as usize);
                    buf.put_slice(&self.to_bytes_be()[zeros..]);
                    32 - zeros
                }

                fn from_compact(buf: &[u8], len: usize) -> (Self, &[u8]) {
                    if len > 0 {
                        let mut arr = [0u8; 32];
                        arr[32 - len..].copy_from_slice(&buf[..len]);
                        (FieldElement::from_bytes_be(&arr).unwrap().into(), &buf[len..])
                    } else {
                        (FieldElement::ZERO.into(), buf)
                    }
                }
            }
        )*
    }
}

impl_compact_for_uints!(u64);
impl_compact_felt!(FieldElement, ContractAddress);

#[cfg(test)]
mod tests {
    use katana_primitives::FieldElement;

    use crate::Compact;

    #[test]
    fn felt_compact() {
        let mut compacted = vec![];
        let value = FieldElement::from(124123137u128);
        let compacted_size = value.to_compact(&mut compacted);
        let (uncompacted, _) = FieldElement::from_compact(&compacted, compacted_size);
        assert_eq!(value, uncompacted);
    }

    #[test]
    fn uint_compact() {
        let mut compacted = vec![];
        let value = 1312412337u64;
        let compacted_size = value.to_compact(&mut compacted);
        let (uncompacted, _) = u64::from_compact(&compacted, compacted_size);
        assert_eq!(value, uncompacted);
    }
}
