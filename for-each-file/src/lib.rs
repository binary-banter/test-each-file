use proc_macro::{Ident, Span};
use proc_macro2::{TokenStream, TokenTree};
use std::fs;
use std::path::{Path, PathBuf};

use syn::{parse_macro_input, DeriveInput, Expr, Token};
use quote::{format_ident, quote, ToTokens};
use syn::LitStr;
use syn::parse::{Parse, ParseStream};

#[derive(Debug)]
struct ForEachFile {
    path: String,
    function: Expr
}

impl Parse for ForEachFile {
    fn parse(input: ParseStream) -> syn::Result<Self> {


        let path = input.parse::<LitStr>()?;
        input.parse::<Token![,]>()?;
        let function = input.parse::<Expr>()?;

        Ok(ForEachFile {
            path: path.value(),
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
    for entry in fs::read_dir(parsed.path).expect("Expected given path to be a directory.") {
        if let Ok(entry) = entry {
            let path = entry.path();
            if path.is_file() {


                let file_name = path.file_stem().expect("Expected file to have a name.").to_str().unwrap().to_owned()
                    .replace('.', "_");
                let file_name = format_ident!("{}", file_name);

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
    }

    proc_macro::TokenStream::from(tokens)
}
