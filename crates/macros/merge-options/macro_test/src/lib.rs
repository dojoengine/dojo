#![allow(clippy::bool_assert_comparison)]

use merge_options::MergeOptions;

#[derive(Debug, MergeOptions)]
pub struct SimpleOptions {
    pub field1: String,
    pub field2: Option<u32>,
    pub field3: bool,
}

impl Default for SimpleOptions {
    fn default() -> Self {
        Self { field1: "default".to_string(), field2: None, field3: false }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_merge() {
        let mut base = SimpleOptions { field1: "base".to_string(), field2: None, field3: true };

        let override_opts =
            SimpleOptions { field1: "override".to_string(), field2: Some(42), field3: false };

        base.merge(Some(&override_opts));

        assert_eq!(base.field1, "base");
        assert_eq!(base.field2, Some(42));
        assert_eq!(base.field3, true);
    }

    #[test]
    fn test_other_none() {
        let mut base = SimpleOptions { field1: "base".to_string(), field2: None, field3: true };

        base.merge(None);

        assert_eq!(base.field1, "base");
        assert_eq!(base.field2, None);
        assert_eq!(base.field3, true);
    }

    #[test]
    fn test_other_override() {
        let mut base = SimpleOptions::default();

        let override_opts =
            SimpleOptions { field1: "override".to_string(), field2: Some(42), field3: true };

        base.merge(Some(&override_opts));

        assert_eq!(base.field1, "override");
        assert_eq!(base.field2, Some(42));
        assert_eq!(base.field3, true);
    }
}
