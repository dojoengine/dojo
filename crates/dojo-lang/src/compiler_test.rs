use dojo_test_utils::compiler::build_test_config;
use scarb::core::TargetKind;
use scarb::ops::{CompileOpts, FeaturesOpts, FeaturesSelector};

use crate::scarb_internal;

#[test]
fn test_compiler_cairo_features() {
    let config = build_test_config("./src/manifest_test_data/compiler_cairo/Scarb.toml").unwrap();

    let features_opts = FeaturesOpts {
        features: FeaturesSelector::AllFeatures,
        no_default_features: false,
    };

    let compile_info = scarb_internal::compile_workspace(
        &config,
        CompileOpts { include_targets: vec![], exclude_targets: vec![TargetKind::TEST], features: features_opts },
    )
    .unwrap();

    assert_eq!(compile_info.compile_error_units, Vec::<String>::default());
}
