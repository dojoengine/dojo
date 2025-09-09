use std::time::Instant;

use scarb_interop::Scarb;

// Criterion enforces at least 10 iterations, in the case of Sozo, we only need to compile the code
// once to have a baseline and compiling 10 times would have been too long for the CI.
// We also output the result in the `bencher` format which is the same as the one used in the
// `bench.yml` action.
fn build_spawn_and_move() {
    Scarb::build_simple_dev("../../examples/spawn-and-move/Scarb.toml".into())
        .expect("Failed to build spawn and move");
}

fn main() {
    // Build a first time to compile the dojo macros.
    build_spawn_and_move();

    // Start the counter now that we are re-building without the dojo macros, only cairo code.
    // Now that we are relying directly on Scarb, this bench makes less sense.
    let start = Instant::now();
    build_spawn_and_move();
    let duration = start.elapsed();

    println!("test build/Sozo.Cold ... bench:     {} ns/iter (+/- 0)", duration.as_nanos());
}
