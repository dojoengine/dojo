use std::sync::Arc;

use cairo_lang_compiler::db::RootDatabaseBuilder;
use cairo_lang_plugins::get_default_plugins;
use cairo_lang_starknet::plugin::StarkNetPlugin;

use crate::plugin::DojoPlugin;

pub trait DojoRootDatabaseBuilderEx {
    /// Tunes a compiler database to Dojo (e.g. Dojo plugin).
    fn with_dojo(&mut self) -> &mut Self;

    /// Tunes a compiler database to Dojo and Starknet
    fn with_dojo_and_starknet(&mut self) -> &mut Self;
}

impl DojoRootDatabaseBuilderEx for RootDatabaseBuilder {
    fn with_dojo(&mut self) -> &mut Self {

        let mut plugins = get_default_plugins();
        plugins.push(Arc::new(DojoPlugin {}));

        self.with_plugins(plugins)
    }

    fn with_dojo_and_starknet(&mut self) -> &mut Self {

        let precedence = ["Pedersen", "RangeCheck", "Bitwise", "EcOp", "GasBuiltin", "System"];

        let mut plugins = get_default_plugins();
        plugins.push(Arc::new(DojoPlugin {}));
        plugins.push(Arc::new(StarkNetPlugin {}));

        self.with_implicit_precedence(&precedence).with_plugins(plugins)
    }
}

pub trait StarknetRootDatabaseBuilderEx {
    /// Tunes a compiler database to StarkNet (e.g. StarkNet plugin).
    fn with_starknet(&mut self) -> &mut Self;
}

impl StarknetRootDatabaseBuilderEx for RootDatabaseBuilder {
    fn with_starknet(&mut self) -> &mut Self {
        // Override implicit precedence for compatibility with the StarkNet OS.
        let precedence = ["Pedersen", "RangeCheck", "Bitwise", "EcOp", "GasBuiltin", "System"];

        let mut plugins = get_default_plugins();
        plugins.push(Arc::new(StarkNetPlugin {}));

        self.with_implicit_precedence(&precedence).with_plugins(plugins)
    }
}