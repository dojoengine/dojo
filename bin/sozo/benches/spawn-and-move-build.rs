use std::time::Instant;

use dojo_lang::scarb_internal::compile_workspace;
use dojo_test_utils::compiler::CompilerTestSetup;
use scarb::compiler::Profile;
use scarb::core::TargetKind;
use scarb::ops::{CompileOpts, FeaturesOpts, FeaturesSelector};

// Criterion enforces at least 10 iterations, in the case of Sozo, we only need to compile the code
// once to have a baseline and compiling 10 times would have been too long for the CI.
// We also output the result in the `bencher` format which is the same as the one used in the
// `bench.yml` action.

fn build_spawn_and_move() {
    let setup = CompilerTestSetup::from_examples("../../crates/dojo-core", "../../examples/");

    let config = setup.build_test_config("spawn-and-move", Profile::DEV);

    let ws = scarb::ops::read_workspace(config.manifest_path(), &config)
        .expect("Failed to read workspace");

    let packages: Vec<_> = ws.members().collect();

    let _compile_info = compile_workspace(
        &config,
        CompileOpts {
            include_target_names: vec![],
            include_target_kinds: vec![],
            exclude_target_kinds: vec![TargetKind::TEST],
            features: FeaturesOpts {
                features: FeaturesSelector::AllFeatures,
                no_default_features: false,
            },
        },
        packages.iter().map(|p| p.id).collect(),
    )
    .expect("Failed to build spawn and move");
}

fn main() {
    let start = Instant::now();
    build_spawn_and_move();
    let duration = start.elapsed();

    println!("test build/Sozo.Cold ... bench:     {} ns/iter (+/- 0)", duration.as_nanos());
}
