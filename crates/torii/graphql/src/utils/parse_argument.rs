use async_graphql::dynamic::ResolverContext;
use async_graphql::Result;

pub trait ParseArgument: Sized {
    fn parse(ctx: &ResolverContext<'_>, input: &str) -> Result<Self>;
}

impl ParseArgument for u64 {
    fn parse(ctx: &ResolverContext<'_>, input: &str) -> Result<Self> {
        let arg = ctx.args.try_get(input);
        arg?.u64()
    }
}

impl ParseArgument for String {
    fn parse(ctx: &ResolverContext<'_>, input: &str) -> Result<Self> {
        let arg = ctx.args.try_get(input);
        Ok(arg?.string()?.to_string())
    }
}
