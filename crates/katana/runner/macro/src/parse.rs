use proc_macro2::{Span, TokenStream, TokenTree};
use quote::TokenStreamExt;
use quote::{quote, quote_spanned, ToTokens};
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{braced, Attribute, Ident, Signature, Visibility};

use crate::config::Configuration;

#[derive(Clone)]
pub struct ItemFn {
    pub outer_attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub sig: Signature,
    pub brace_token: syn::token::Brace,
    pub inner_attrs: Vec<Attribute>,
    pub stmts: Vec<proc_macro2::TokenStream>,
}

impl ItemFn {
    /// Access all attributes of the function item.
    fn attrs(&self) -> impl Iterator<Item = &Attribute> {
        self.outer_attrs.iter().chain(self.inner_attrs.iter())
    }

    /// Get the body of the function item in a manner so that it can be
    /// conveniently used with the `quote!` macro.
    fn body(&self) -> Body<'_> {
        Body { brace_token: self.brace_token, stmts: &self.stmts }
    }

    /// Convert our local function item into a token stream.
    fn into_tokens(
        mut self,
        generated_attrs: proc_macro2::TokenStream,
        // func: proc_macro2::TokenStream,
        last_block: proc_macro2::TokenStream,
    ) -> TokenStream {
        self.sig.asyncness = None;
        // empty out the arguments
        self.sig.inputs.clear();

        // remove duplicate outer attributes
        self.outer_attrs.dedup_by(|a, b| a.path() == b.path());

        let mut tokens = proc_macro2::TokenStream::new();
        // Outer attributes are simply streamed as-is.
        for attr in self.outer_attrs {
            attr.to_tokens(&mut tokens);
        }

        // Inner attributes require extra care, since they're not supported on
        // blocks (which is what we're expanded into) we instead lift them
        // outside of the function. This matches the behavior of `syn`.
        for mut attr in self.inner_attrs {
            attr.style = syn::AttrStyle::Outer;
            attr.to_tokens(&mut tokens);
        }

        // Add generated macros at the end, so macros processed later are aware of them.
        generated_attrs.to_tokens(&mut tokens);

        self.vis.to_tokens(&mut tokens);
        self.sig.to_tokens(&mut tokens);

        self.brace_token.surround(&mut tokens, |tokens| {
            // func.to_tokens(tokens);
            last_block.to_tokens(tokens);
        });

        tokens
    }
}

impl Parse for ItemFn {
    #[inline]
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        // This parse implementation has been largely lifted from `syn`, with
        // the exception of:
        // * We don't have access to the plumbing necessary to parse inner attributes in-place.
        // * We do our own statements parsing to avoid recursively parsing entire statements and
        //   only look for the parts we're interested in.

        let outer_attrs = input.call(Attribute::parse_outer)?;
        let vis: Visibility = input.parse()?;
        let sig: Signature = input.parse()?;

        let content;
        let brace_token = braced!(content in input);
        let inner_attrs = Attribute::parse_inner(&content)?;

        let mut buf = proc_macro2::TokenStream::new();
        let mut stmts = Vec::new();

        while !content.is_empty() {
            if let Some(semi) = content.parse::<Option<syn::Token![;]>>()? {
                semi.to_tokens(&mut buf);
                stmts.push(buf);
                buf = proc_macro2::TokenStream::new();
                continue;
            }

            // Parse a single token tree and extend our current buffer with it.
            // This avoids parsing the entire content of the sub-tree.
            buf.extend([content.parse::<TokenTree>()?]);
        }

        if !buf.is_empty() {
            stmts.push(buf);
        }

        Ok(Self { outer_attrs, vis, sig, brace_token, inner_attrs, stmts })
    }
}

struct Body<'a> {
    brace_token: syn::token::Brace,
    // Statements, with terminating `;`.
    stmts: &'a [TokenStream],
}

impl ToTokens for Body<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.brace_token.surround(tokens, |tokens| {
            for stmt in self.stmts {
                stmt.to_tokens(tokens);
            }
        });
    }
}

pub fn parse_knobs(mut input: ItemFn, is_test: bool, config: Configuration) -> TokenStream {
    // If type mismatch occurs, the current rustc points to the last statement.
    let (last_stmt_start_span, last_stmt_end_span) = {
        let mut last_stmt = input.stmts.last().cloned().unwrap_or_default().into_iter();

        // `Span` on stable Rust has a limitation that only points to the first
        // token, not the whole tokens. We can work around this limitation by
        // using the first/last span of the tokens like
        // `syn::Error::new_spanned` does.
        let start = last_stmt.next().map_or_else(Span::call_site, |t| t.span());
        let end = last_stmt.last().map_or(start, |t| t.span());
        (start, end)
    };

    let crate_path = config
        .crate_name
        .map(ToTokens::into_token_stream)
        .unwrap_or_else(|| Ident::new("katana_runner", last_stmt_start_span).into_token_stream());

    let mut cfg: TokenStream = quote! {};

    if let Some(value) = config.block_time {
        cfg = quote_spanned! (last_stmt_start_span=> #cfg block_time: Some(#value), );
    }

    if let Some(value) = config.fee {
        cfg = quote_spanned! (last_stmt_start_span=> #cfg disable_fee: #value, );
    }

    if let Some(value) = config.db_dir {
        cfg = quote_spanned! (last_stmt_start_span=> #cfg db_dir: Some(#value), );
    }

    if let Some(value) = config.accounts {
        cfg = quote_spanned! (last_stmt_start_span=> #cfg n_accounts: #value, );
    }

    if let Some(value) = config.log_path {
        cfg = quote_spanned! (last_stmt_start_span=> #cfg, log_path: Some(#value), );
    }

    if config.dev {
        cfg = quote_spanned! (last_stmt_start_span=> #cfg dev: true, );
    }

    cfg = quote_spanned! {last_stmt_start_span=>
        #crate_path::KatanaRunnerConfig { #cfg ..Default::default() }
    };

    let generated_attrs = if is_test {
        quote! {
            #[::core::prelude::v1::test]
        }
    } else {
        quote! {}
    };

    let body = input.body();
    let body = if input.sig.asyncness.is_some() {
        quote! {
            struct RunnerCtx(#crate_path::KatanaRunner);

            impl core::ops::Deref for RunnerCtx {
                type Target = #crate_path::KatanaRunner;
                fn deref(&self) -> &Self::Target {
                    &self.0
                }
            }

            let body = async #body;
        }
    } else {
        quote! {
            struct RunnerCtx(#crate_path::KatanaRunner);

            impl core::ops::Deref for RunnerCtx {
                type Target = #crate_path::KatanaRunner;
                fn deref(&self) -> &Self::Target {
                    &self.0
                }
            }

            let body = || #body;
        }
    };

    let last_block = quote_spanned! {last_stmt_end_span=>
        {
            let runner = #crate_path::KatanaRunner::new_with_config(#cfg).expect("Failed to start runner.");
            let ctx = RunnerCtx(runner);
            #body
            return body();
        }
    };

    input.into_tokens(generated_attrs, last_block)
}

pub fn parse_string(
    int: syn::Lit,
    span: proc_macro2::Span,
    field: &str,
) -> Result<String, syn::Error> {
    match int {
        syn::Lit::Str(s) => Ok(s.value()),
        syn::Lit::Verbatim(s) => Ok(s.to_string()),
        _ => Err(syn::Error::new(span, format!("Failed to parse value of `{field}` as string."))),
    }
}

pub fn parse_path(
    lit: syn::Lit,
    span: proc_macro2::Span,
    field: &str,
) -> Result<syn::Path, syn::Error> {
    match lit {
        syn::Lit::Str(s) => {
            let err = syn::Error::new(
                span,
                format!("Failed to parse value of `{field}` as path: \"{}\"", s.value()),
            );
            s.parse::<syn::Path>().map_err(|_| err.clone())
        }
        _ => Err(syn::Error::new(span, format!("Failed to parse value of `{}` as path.", field))),
    }
}

pub fn parse_bool(
    bool: syn::Lit,
    span: proc_macro2::Span,
    field: &str,
) -> Result<bool, syn::Error> {
    match bool {
        syn::Lit::Bool(b) => Ok(b.value),
        _ => Err(syn::Error::new(span, format!("Failed to parse value of `{field}` as bool."))),
    }
}

pub fn parse_int(int: syn::Lit, span: proc_macro2::Span, field: &str) -> Result<usize, syn::Error> {
    match int {
        syn::Lit::Int(lit) => match lit.base10_parse::<usize>() {
            Ok(value) => Ok(value),
            Err(e) => Err(syn::Error::new(
                span,
                format!("Failed to parse value of `{field}` as integer: {e}"),
            )),
        },
        _ => Err(syn::Error::new(span, format!("Failed to parse value of `{field}` as integer."))),
    }
}
