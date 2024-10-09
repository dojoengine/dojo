#![allow(unused)]

use proc_macro2::{Span, TokenStream, TokenTree};
use quote::{quote, quote_spanned, ToTokens, TokenStreamExt};
use syn::parse::{Parse, ParseStream};
use syn::{braced, parse_quote, Attribute, Ident, Signature, Visibility};

use crate::config::Configuration;
use crate::utils::{attr_ends_with, parse_bool, parse_int, parse_string};

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
    pub fn attrs(&self) -> impl Iterator<Item = &Attribute> {
        self.outer_attrs.iter().chain(self.inner_attrs.iter())
    }

    /// Get the body of the function item in a manner so that it can be
    /// conveniently used with the `quote!` macro.
    pub fn body(&self) -> Body<'_> {
        Body { brace_token: self.brace_token, stmts: &self.stmts }
    }

    /// Convert our local function item into a token stream.
    pub fn into_tokens(
        mut self,
        generated_attrs: proc_macro2::TokenStream,
        inner: proc_macro2::TokenStream,
        last_block: proc_macro2::TokenStream,
    ) -> TokenStream {
        self.sig.asyncness = None;
        // empty out the arguments
        self.sig.inputs.clear();

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
            inner.to_tokens(tokens);
            last_block.to_tokens(tokens);
        });

        tokens
    }
}

impl ToTokens for ItemFn {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.append_all(self.outer_attrs.iter());
        self.vis.to_tokens(tokens);
        self.sig.to_tokens(tokens);
        self.brace_token.surround(tokens, |tokens| {
            tokens.append_all(self.inner_attrs.iter());
            tokens.append_all(&self.stmts);
        });
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

pub struct Body<'a> {
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
