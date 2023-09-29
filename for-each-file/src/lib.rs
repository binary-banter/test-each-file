
use proc_macro2::{Ident, TokenStream, TokenTree};
use std::fs;
use std::path::{Path, PathBuf};
use itertools::Itertools;
use pathdiff::diff_paths;

use syn::{parse_macro_input, DeriveInput, Expr, Token};
use quote::{format_ident, quote, ToTokens};
use syn::LitStr;
use syn::parse::{Parse, ParseStream};
use syn::parse::discouraged::Speculative;
use walkdir::WalkDir;

#[derive(Debug)]
struct ForEachFile {
    path: String,
    prefix: Option<Ident>,
    function: Expr
}

impl Parse for ForEachFile {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let input1 = input.fork();

        if let Ok(ok) = ForEachFile::parse_with_prefix(&input1)  {
            input.advance_to(&input1);
            return Ok(ok);
        }

        ForEachFile::parse_without_prefix(input)
    }
}

impl ForEachFile {
    fn parse_with_prefix(input: ParseStream) -> syn::Result<Self> {
        let path = input.parse::<LitStr>()?.value();
        input.parse::<Token![,]>()?;

        let prefix = input.parse::<Ident>()?;
        input.parse::<Token![,]>()?;

        let function = input.parse::<Expr>()?;

        Ok(ForEachFile {
            path,
            prefix: Some(prefix),
            function,
        })
    }

    fn parse_without_prefix(input: ParseStream) -> syn::Result<Self> {
        let path = input.parse::<LitStr>()?.value();
        input.parse::<Token![,]>()?;

        let function = input.parse::<Expr>()?;

        Ok(ForEachFile {
            path,
            prefix: None,
            function,
        })
    }
}

/// Example of [function-like procedural macro][1].
///
/// [1]: https://doc.rust-lang.org/reference/procedural-macros.html#function-like-procedural-macros
#[proc_macro]
pub fn test_each_file(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let parsed = parse_macro_input!(input as ForEachFile);

    let mut tokens = TokenStream::new();

    for entry in WalkDir::new(&parsed.path).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            let mut diff = diff_paths(path, &parsed.path).unwrap();
            diff.set_extension("");

            let file_name = diff.components().map(|c| c.as_os_str().to_str().expect("Expected file names to be UTF-8.")).format("_");
            let file_name = if let Some(prefix) = &parsed.prefix {
                format_ident!("{prefix}_{file_name}")
            } else {
                format_ident!("{file_name}")
            };

            let function = &parsed.function;

            let content = fs::read_to_string(path).expect("Expected reading file to be successful.");
            tokens.extend(quote! {
                    #[test]
                    fn #file_name() {
                        (#function)(#content)
                    }
                });
        }
    }

    proc_macro::TokenStream::from(tokens)
}
