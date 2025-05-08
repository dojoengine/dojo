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

#[derive(Debug, MergeOptions)]
pub struct NestedOptions {
    pub field1: String,
    pub field2: Option<u32>,
    pub field3: bool,
    pub inner: MyInner,
}

impl Default for NestedOptions {
    fn default() -> Self {
        Self { field1: "default".to_string(), field2: None, field3: false, inner: MyInner::default() }
    }
}

#[derive(Debug, MergeOptions)]
pub struct MyInner {
    pub timeout: u32,
    pub retries: u8,
}

impl Default for MyInner {
    fn default() -> Self {
        Self { timeout: 30, retries: 3 }
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

    #[test]
    fn test_nested_merge() {
        let mut base = NestedOptions::default();
        
        let override_opts = NestedOptions {
            field1: "override".to_string(),
            field2: Some(42),
            field3: true,
            inner: MyInner { timeout: 2000, retries: 5 },
        };

        base.merge(Some(&override_opts));

        assert_eq!(base.field1, "override");
        assert_eq!(base.field2, Some(42));
        assert_eq!(base.field3, true);
        assert_eq!(base.inner.timeout, 2000);
        assert_eq!(base.inner.retries, 5);
    }

    #[test]
    fn test_nested_partial_override() {
        let mut base = NestedOptions {
            field1: "base".to_string(),
            field2: Some(10),
            field3: false,
            inner: MyInner { timeout: 30, retries: 10 },
        };
        
        let override_opts = NestedOptions {
            field1: "override".to_string(),
            field2: Some(42),
            field3: true,
            inner: MyInner { timeout: 2000, retries: 5 },
        };

        base.merge(Some(&override_opts));

        assert_eq!(base.field1, "base");
        assert_eq!(base.field2, Some(10));
        assert_eq!(base.field3, true);
        assert_eq!(base.inner.timeout, 2000);
        assert_eq!(base.inner.retries, 10);
    }

    #[test]
    fn test_inner_struct_direct() {
        let mut base = MyInner { timeout: 30, retries: 10 };
        
        let override_opts = MyInner { timeout: 2000, retries: 5 };

        base.merge(Some(&override_opts));

        assert_eq!(base.timeout, 2000);
        assert_eq!(base.retries, 10);
    }
}
