// This is a partial implementation of https://github.com/starknet-io/types-rs/pull/74
// and is required because signed integers are not coverted from Felt correctly with the
// current implementation
// TODO: remove when https://github.com/starknet-io/types-rs/pull/74 is merged.

use core::convert::TryInto;

use starknet::core::types::Felt;

#[derive(Debug, Copy, Clone)]
pub struct PrimitiveFromFeltError;

impl core::fmt::Display for PrimitiveFromFeltError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Failed to convert `Felt` into primitive type")
    }
}

impl std::error::Error for PrimitiveFromFeltError {
    fn description(&self) -> &str {
        "Failed to convert `Felt` into primitive type"
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

#[cfg(test)]
mod tests {
    use starknet::core::types::Felt;

    use super::try_from_felt;

    #[test]
    fn test_try_from_felt() {
        let i_8: i8 = -64;
        let felt = Felt::from(i_8);
        let signed_integer = try_from_felt::<i8>(felt).unwrap();
        assert_eq!(i_8, signed_integer);

        let i_16: i16 = -14293;
        let felt = Felt::from(i_16);
        let signed_integer = try_from_felt::<i16>(felt).unwrap();
        assert_eq!(i_16, signed_integer);

        let i_32: i32 = -194875;
        let felt = Felt::from(i_32);
        let signed_integer = try_from_felt::<i32>(felt).unwrap();
        assert_eq!(i_32, signed_integer);

        let i_64: i64 = -3147483648;
        let felt = Felt::from(i_64);
        let signed_integer = try_from_felt::<i64>(felt).unwrap();
        assert_eq!(i_64, signed_integer);

        let i_128: i128 = -170141183460469231731687303715884105728;
        let felt = Felt::from(i_128);
        let signed_integer = try_from_felt::<i128>(felt).unwrap();
        assert_eq!(i_128, signed_integer);
    }
}
