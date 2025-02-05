use crate::Felt;

/// A Cairo short string.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ShortString(heapless::String<31>);

impl ShortString {
    /// Creates a new empty short string.
    pub const fn new() -> Self {
        Self(heapless::String::new())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[inline]
    fn push(&mut self, c: char) -> Result<(), ()> {
        self.0.push(c)
    }

    #[inline]
    fn push_str(&mut self, string: &str) -> Result<(), ()> {
        self.0.push_str(string)
    }
}

impl Default for ShortString {
    fn default() -> Self {
        Self::new()
    }
}

impl core::ops::Deref for ShortString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_str()
    }
}

impl AsRef<str> for ShortString {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ShortStringTryFromStrError {
    #[error("String is too long to be a Cairo short string")]
    StringTooLong,
    #[error("Invalid ASCII character in string")]
    InvalidAsciiString,
}

impl core::str::FromStr for ShortString {
    type Err = ShortStringTryFromStrError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.is_ascii() {
            return Err(ShortStringTryFromStrError::InvalidAsciiString);
        }

        if s.len() > 31 {
            return Err(ShortStringTryFromStrError::StringTooLong);
        }

        let mut string = Self::new();
        string.push_str(s).expect("length already checked");

        Ok(string)
    }
}

impl From<ShortString> for String {
    fn from(string: ShortString) -> Self {
        string.0.to_string()
    }
}

impl From<ShortString> for Felt {
    fn from(string: ShortString) -> Self {
        Self::from(&string)
    }
}

impl From<&ShortString> for Felt {
    fn from(string: &ShortString) -> Self {
        Felt::from_bytes_be_slice(string.0.as_bytes())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ShortStringFromFeltError {
    #[error("Unexpected null terminator in string")]
    UnexpectedNullTerminator,
    #[error("String exceeds maximum length for Cairo short strings")]
    StringTooLong,
    #[error("Non-ASCII character found")]
    NonAsciiCharacter,
}

impl TryFrom<Felt> for ShortString {
    type Error = ShortStringFromFeltError;

    fn try_from(value: Felt) -> Result<Self, Self::Error> {
        if value == Felt::ZERO {
            return Ok(Self::new());
        }

        let bytes = value.to_bytes_be();

        // First byte must be zero because the string must only be 31 bytes.
        if bytes[0] > 0 {
            return Err(ShortStringFromFeltError::StringTooLong);
        }

        let mut string = ShortString::new();

        for byte in bytes {
            if byte == 0u8 {
                if !string.is_empty() {
                    return Err(ShortStringFromFeltError::UnexpectedNullTerminator);
                }
            } else if byte.is_ascii() {
                string.push(byte as char).expect("qed; should fit");
            } else {
                return Err(ShortStringFromFeltError::NonAsciiCharacter);
            }
        }

        Ok(string)
    }
}

impl TryFrom<&Felt> for ShortString {
    type Error = ShortStringFromFeltError;

    fn try_from(value: &Felt) -> Result<Self, Self::Error> {
        Self::try_from(*value)
    }
}

impl core::fmt::Display for ShortString {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for ShortString {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let mut raw_bytes = heapless::Vec::<u8, 31>::new();
        let length = u.int_in_range(0..=31)?;

        for _ in 0..length {
            let char = u.int_in_range(0..=127)?; // ASCII range
            raw_bytes.push(char).expect("shouldn't be full");
        }

        let str = heapless::String::<31>::from_utf8(raw_bytes).expect("should be valid utf8");
        Ok(Self(str))
    }
}

#[cfg(test)]
mod tests {
    use core::str::FromStr;

    use assert_matches::assert_matches;

    use super::ShortString;
    use crate::cairo::ShortStringFromFeltError;
    use crate::Felt;

    #[test]
    fn new_short_string_is_empty() {
        let s = ShortString::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
        assert_eq!(s.as_str(), "");
    }

    #[test]
    fn try_from_valid_str() {
        let s = ShortString::from_str("hello").unwrap();
        assert_eq!(s.as_str(), "hello");
        assert_eq!(s.len(), 5);
    }

    #[test]
    fn try_from_invalid_str_too_long() {
        let long_str = "a".repeat(32);
        assert!(ShortString::from_str(long_str.as_str()).is_err());
    }

    #[test]
    fn round_trip_felt() {
        let original = ShortString::from_str("abc").unwrap();
        let felt = Felt::from(original.clone());
        let converted = ShortString::try_from(felt).unwrap();
        assert_eq!(original, converted);
    }

    #[test]
    fn felt_with_non_zero_first_byte() {
        // Create felt with non-zero first byte
        let mut bytes = [0u8; 32];
        bytes[0] = 1;
        let felt = Felt::from_bytes_be(&bytes);
        assert_matches!(ShortString::try_from(felt), Err(ShortStringFromFeltError::StringTooLong));
    }

    #[test]
    fn felt_with_valid_string() {
        let mut bytes = [0u8; 32];
        bytes[27..32].copy_from_slice(b"hello");
        let felt = Felt::from_bytes_be(&bytes);
        let s = ShortString::try_from(felt).unwrap();
        assert_eq!(s.as_str(), "hello");
    }

    #[test]
    fn felt_with_trailing_non_zero() {
        let mut bytes = [0u8; 32];
        bytes[31] = b'a';
        let felt = Felt::from_bytes_be(&bytes);
        let s = ShortString::try_from(felt).unwrap();
        assert_eq!(s.as_str(), "a");
    }

    #[test]
    fn felt_with_max_length() {
        let mut bytes = [0u8; 32];
        let s = "a".repeat(31);
        bytes[1..].copy_from_slice(s.as_bytes());
        let felt = Felt::from_bytes_be(&bytes);
        let result = ShortString::try_from(felt).unwrap();
        assert_eq!(result.len(), 31);
        assert_eq!(result.as_str(), s);
    }

    #[test]
    fn felt_zero() {
        let s = ShortString::try_from(Felt::ZERO).unwrap();
        assert!(s.is_empty());
    }

    #[rstest::rstest]
    #[case({
        let mut bytes = [0u8; 32];
        bytes[1] = b'a';
        bytes[2] = 0;
        bytes[3] = b'b';
        bytes
    })]
    #[case({
        let mut bytes = [0u8; 32];
        bytes[1] = b'a';
        bytes[2] = 0;
        bytes
    })]
    fn test_felt_with_null(#[case] bytes: [u8; 32]) {
        let felt = Felt::from_bytes_be(&bytes);
        assert!(matches!(
            ShortString::try_from(felt),
            Err(ShortStringFromFeltError::UnexpectedNullTerminator)
        ));
    }

    #[cfg(feature = "arbitrary")]
    #[test]
    fn test_arbitrary_short_string() {
        use arbitrary::{Arbitrary, Unstructured};

        let data = vec![0u8; 128];
        let mut u = Unstructured::new(&data);

        for _ in 0..100 {
            let s = ShortString::arbitrary(&mut u).unwrap();
            assert!(s.len() <= 31);
            assert!(String::from(s).into_bytes().into_iter().all(|b| b <= 127));
        }
    }
}
