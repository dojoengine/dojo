use async_graphql::dynamic::ResolverContext;
use async_graphql::Result;

pub trait ParseArgument: Sized {
    fn parse(ctx: &ResolverContext<'_>, input: String) -> Result<Self>;
}

impl ParseArgument for u64 {
    fn parse(ctx: &ResolverContext<'_>, input: String) -> Result<Self> {
        let arg = ctx.args.try_get(input.as_str());
        arg?.u64()
    }
}

impl ParseArgument for String {
    fn parse(ctx: &ResolverContext<'_>, input: String) -> Result<Self> {
        let arg = ctx.args.try_get(input.as_str());
        Ok(arg?.string()?.to_string())
    }
}
