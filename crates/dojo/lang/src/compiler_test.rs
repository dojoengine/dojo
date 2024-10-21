use dojo_test_utils::compiler::build_test_config;
use scarb::compiler::Profile;
use scarb::core::{PackageName, TargetKind};
use scarb::ops::{CompileOpts, FeaturesOpts, FeaturesSelector};

use crate::compiler::ContractSelector;
use crate::scarb_internal;

// Ignored as scarb takes too much time to compile in debug mode.
// It's anyway run in the CI in the `test` job.
#[test]
#[ignore]
fn test_compiler_cairo_features() {
    let config =
        build_test_config("./src/manifest_test_data/compiler_cairo/Scarb.toml", Profile::DEV)
            .unwrap();

    let features_opts =
        FeaturesOpts { features: FeaturesSelector::AllFeatures, no_default_features: false };
    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();
    let packages: Vec<scarb::core::PackageId> = ws.members().map(|p| p.id).collect();

    let compile_info = scarb_internal::compile_workspace(
        &config,
        CompileOpts {
            include_target_names: vec![],
            include_target_kinds: vec![],
            exclude_target_kinds: vec![TargetKind::TEST],
            features: features_opts,
        },
        packages,
    )
    .unwrap();

    assert_eq!(compile_info.compile_error_units, Vec::<String>::default());
}

#[test]
fn test_package() {
    let selector = ContractSelector("my_package::my_contract".to_string());
    assert_eq!(selector.package(), PackageName::new("my_package"));

    let selector_no_separator = ContractSelector("my_package".to_string());
    assert_eq!(selector_no_separator.package(), PackageName::new("my_package"));
}

#[test]
fn test_path_with_model_snake_case() {
    let selector = ContractSelector("my_package::MyContract".to_string());
    assert_eq!(selector.path_with_model_snake_case(), "my_package::my_contract");

    let selector_multiple_segments =
        ContractSelector("my_package::sub_package::MyContract".to_string());
    assert_eq!(
        selector_multiple_segments.path_with_model_snake_case(),
        "my_package::sub_package::my_contract"
    );

    // In snake case, erc20 should be erc_20. This test ensures that the path is converted to snake
    // case only for the model's name.
    let selector_erc20 = ContractSelector("my_package::erc20::Token".to_string());
    assert_eq!(selector_erc20.path_with_model_snake_case(), "my_package::erc20::token");
}
