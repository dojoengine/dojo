use core::convert::TryInto;
use starknet::core::types::Felt;

#[derive(Debug, Copy, Clone)]
pub struct PrimitiveFromFeltError;

impl core::fmt::Display for PrimitiveFromFeltError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Failed to convert `Felt` into primitive type")
    }
}

const MINUS_TWO_BYTES_REPR: [u8; 32] = [
    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
    255, 255, 255, 255, 255, 16, 0, 0, 0, 0, 0, 0, 8,
];

pub trait FromFelt: Sized {
    fn try_from_felt(value: Felt) -> Result<Self, PrimitiveFromFeltError>;
}

macro_rules! impl_from_felt {
    ($into:ty) => {
        impl FromFelt for $into {
            fn try_from_felt(value: Felt) -> Result<Self, PrimitiveFromFeltError> {
                let size_of_type = core::mem::size_of::<$into>();
                let bytes_le = value.to_bytes_le();

                if bytes_le[size_of_type..].iter().all(|&v| v == 0)
                    && bytes_le[size_of_type - 1] <= 0b01111111
                {
                    Ok(<$into>::from_le_bytes(bytes_le[..size_of_type].try_into().unwrap()))
                } else if bytes_le[size_of_type..] == MINUS_TWO_BYTES_REPR[size_of_type..]
                    && bytes_le[size_of_type - 1] >= 0b10000000
                {
                    let offsetted_value =
                        <$into>::from_le_bytes(bytes_le[..size_of_type].try_into().unwrap());

                    offsetted_value.checked_sub(1).ok_or(PrimitiveFromFeltError)
                } else if bytes_le[24..] == [17, 0, 0, 0, 0, 0, 0, 8] {
                    return Ok(-1);
                } else {
                    Err(PrimitiveFromFeltError)
                }
            }
        }
    };
}

impl_from_felt!(i8);
impl_from_felt!(i16);
impl_from_felt!(i32);
impl_from_felt!(i64);
impl_from_felt!(i128);

pub fn try_from_felt<T: FromFelt>(value: Felt) -> Result<T, PrimitiveFromFeltError> {
    T::try_from_felt(value)
}
