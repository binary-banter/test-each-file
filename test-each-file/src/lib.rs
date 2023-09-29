use std::collections::HashSet;
use proc_macro2::{Ident, TokenStream};
use std::fs::canonicalize;
use itertools::Itertools;
use pathdiff::diff_paths;

use syn::{parse_macro_input, Expr, Token, bracketed, LitStr};
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use walkdir::WalkDir;

struct ForEachFile {
    path: String,
    prefix: Option<Ident>,
    function: Expr,
    extensions: Vec<String>,
}

impl Parse for ForEachFile {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let extensions = if input.peek(Token![for]) {
            input.parse::<Token![for]>()?;

            let content;
            bracketed!(content in input);

            let extensions = Punctuated::<LitStr, Token![,]>::parse_terminated(&content)?.into_iter().map(|s| s.value()).collect::<Vec<_>>();
            assert!(!extensions.is_empty(), "Expected at least one extension to be given.");

            extensions
        } else {
            vec![]
        };

        input.parse::<Token![in]>()?;
        let path = input.parse::<LitStr>()?.value();

        let prefix = if input.peek(Token![as]) {
            input.parse::<Token![as]>()?;
            Some(input.parse::<Ident>()?)
        } else {
            None
        };

        input.parse::<Token![=>]>()?;
        let function = input.parse::<Expr>()?;

        Ok(Self {
            path,
            prefix,
            function,
            extensions
        })
    }
}

#[proc_macro]
pub fn test_each_file(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let parsed = parse_macro_input!(input as ForEachFile);

    let mut tokens = TokenStream::new();
    let mut files = HashSet::new();
    for entry in WalkDir::new(&parsed.path).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            let mut file = path.to_path_buf();
            if !parsed.extensions.is_empty() {
                file.set_extension("");
            }
            files.insert(file);
        }
    }

    for file in files {
        let mut diff = diff_paths(&file, &parsed.path).unwrap();
        diff.set_extension("");
        let file_name = diff.components().map(|c| c.as_os_str().to_str().expect("Expected file names to be UTF-8.")).format("_");

        let file_name = if let Some(prefix) = &parsed.prefix {
            format_ident!("{prefix}_{file_name}")
        } else {
            format_ident!("{file_name}")
        };

        let function = &parsed.function;

        let content = if parsed.extensions.is_empty() {
            let file = canonicalize(file).unwrap();
            let file = file.to_str().unwrap();
            quote!(include_str!(#file))
        } else {
            let mut content = TokenStream::new();

            for ext in &parsed.extensions {
                let mut file = file.clone();
                file.set_extension(ext);
                let file = canonicalize(file).unwrap();
                let file = file.to_str().unwrap();

                content.extend(quote!(include_str!(#file),));
            }

            quote!([#content])
        };

        tokens.extend(quote! {
            #[test]
            fn #file_name() {
                (#function)(#content)
            }
        });
    }

    proc_macro::TokenStream::from(tokens)
}
