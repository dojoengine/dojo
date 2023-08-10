fn main() {
    cynic_codegen::register_schema("world")
        .from_sdl_file("src/provider/torii/schema/world.graphql")
        .unwrap()
        .as_default()
        .unwrap();
}
