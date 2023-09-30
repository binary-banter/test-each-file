use pathdiff::diff_paths;
use proc_macro2::{Ident, TokenStream};
use std::collections::{HashMap, HashSet};
use std::fs::canonicalize;
use std::path::{Path, PathBuf};

use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{bracketed, parse_macro_input, Expr, LitStr, Token};

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

            let extensions = Punctuated::<LitStr, Token![,]>::parse_terminated(&content)?
                .into_iter()
                .map(|s| s.value())
                .collect::<Vec<_>>();
            assert!(
                !extensions.is_empty(),
                "Expected at least one extension to be given."
            );

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
            extensions,
        })
    }
}

#[derive(Default)]
struct Tree {
    children: HashMap<PathBuf, Tree>,
    here: HashSet<PathBuf>,
}

impl Tree {
    fn new(base: &Path, ignore_extensions: bool) -> Self {
        assert!(base.is_dir());
        let mut tree = Self::default();
        for entry in base.read_dir().unwrap() {
            let mut entry = entry.unwrap().path();
            if entry.is_file() {
                if ignore_extensions {
                    entry.set_extension("");
                }
                tree.here.insert(entry);
            } else if entry.is_dir() {
                tree.children.insert(
                    entry.as_path().to_path_buf(),
                    Self::new(entry.as_path(), ignore_extensions),
                );
            } else {
                panic!("Unsupported path.")
            }
        }
        tree
    }
}

fn generate_from_tree(tree: &Tree, parsed: &ForEachFile, stream: &mut TokenStream) {
    for file in &tree.here {
        let mut diff = diff_paths(file, &parsed.path).unwrap();
        diff.set_extension("");
        let file_name = diff.file_name().unwrap().to_str().unwrap();

        let file_name = format_ident!("{file_name}");

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

        stream.extend(quote! {
            #[test]
            fn #file_name() {
                (#function)(#content)
            }
        });
    }

    for (name, directory) in &tree.children {
        let mut sub_stream = TokenStream::new();
        generate_from_tree(directory, parsed, &mut sub_stream);
        let name = format_ident!("{}", name.file_name().unwrap().to_str().unwrap());
        stream.extend(quote! {
            mod #name {
                use super::*;
                #sub_stream
            }
        })
    }
}

#[proc_macro]
pub fn test_each_file(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let parsed = parse_macro_input!(input as ForEachFile);

    let mut tokens = TokenStream::new();
    let files = Tree::new(parsed.path.as_ref(), !parsed.extensions.is_empty());
    generate_from_tree(&files, &parsed, &mut tokens);

    if let Some(prefix) = parsed.prefix {
        tokens = quote! {
            #[cfg(test)]
            mod #prefix {
                use super::*;
                #tokens
            }
        }
    }

    proc_macro::TokenStream::from(tokens)
}
