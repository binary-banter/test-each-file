#![doc = include_str!("../README.md")]
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{bracketed, parse_macro_input, Expr, LitStr, Token};

struct ForEachArgs {
    path: LitStr,
    module: Option<Ident>,
    function: Expr,
    extensions: Vec<String>,
}

macro_rules! abort {
    ($span:expr, $message:expr) => {
        return Err(syn::Error::new($span, $message))
    };
}

macro_rules! abort_token_stream {
    ($span:expr, $message:expr) => {
        return syn::Error::new($span, $message).into_compile_error().into()
    };
}

impl Parse for ForEachArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Optionally parse extensions if the keyword `for` is used. Aborts if none are given.
        let extensions = input
            .parse::<Token![for]>()
            .and_then(|_| {
                let content;
                bracketed!(content in input);

                match Punctuated::<LitStr, Token![,]>::parse_separated_nonempty(&content) {
                    Ok(extensions) => Ok(extensions
                        .into_iter()
                        .map(|extension| extension.value())
                        .collect()),
                    Err(e) => abort!(e.span(), "Expected at least one extension to be given."),
                }
            })
            .unwrap_or_default();

        // Parse the path to the tests.
        if let Err(e) = input.parse::<Token![in]>() {
            abort!(e.span(), "Expected the keyword `in` before the path.");
        };

        let path = match input.parse::<LitStr>() {
            Ok(path) => path,
            Err(e) => abort!(e.span(), "Expected a path after the keyword 'in'."),
        };

        // Optionally parse module to put the tests in if the keyword `as` is used.
        let module = input
            .parse::<Token![as]>()
            .and_then(|_| match input.parse::<Ident>() {
                Ok(module) => Ok(module),
                Err(e) => abort!(e.span(), "Expected a module to be given."),
            })
            .ok();

        // Parse function to call.
        if let Err(e) = input.parse::<Token![=>]>() {
            abort!(e.span(), "Expected `=>` before the function to call.");
        };

        let function = match input.parse::<Expr>() {
            Ok(function) => function,
            Err(e) => abort!(e.span(), "Expected a function to call after `=>`."),
        };

        Ok(Self {
            path,
            module,
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
    fn new(base: &Path, ignore_extensions: bool) -> Result<Self, String> {
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
                    Self::new(entry.as_path(), ignore_extensions)?,
                );
            } else {
                return Err(format!("Unsupported path: {:#?}.", entry));
            }
        }
        Ok(tree)
    }
}

enum Type {
    File,
    Path,
}

fn generate_from_tree(
    tree: &Tree,
    parsed: &ForEachArgs,
    stream: &mut TokenStream,
    invocation_type: &Type,
) -> Result<(), String> {
    for file in &tree.here {
        let file_name = format_ident!("{}", file.file_stem().unwrap().to_str().unwrap());

        let function = &parsed.function;

        let arguments = if parsed.extensions.is_empty() {
            let input = file.canonicalize().unwrap();
            let input = input.to_str().unwrap();

            match invocation_type {
                Type::File => quote!(include_str!(#input)),
                Type::Path => quote!(#input),
            }
        } else {
            let mut arguments = TokenStream::new();

            for extension in &parsed.extensions {
                let input = match file.with_extension(extension).canonicalize() {
                    Ok(path) => path,
                    Err(e) => return Err(format!("Failed to read expected file {}.{extension}: {e}", file.display())),
                };
                let input = input.to_str().unwrap();

                arguments.extend(match invocation_type {
                    Type::File => quote!(include_str!(#input),),
                    Type::Path => quote!(#input,),
                });
            }

            quote!([#arguments])
        };

        stream.extend(quote! {
            #[test]
            fn #file_name() {
                (#function)(#arguments)
            }
        });
    }

    for (name, directory) in &tree.children {
        let mut sub_stream = TokenStream::new();
        generate_from_tree(directory, parsed, &mut sub_stream, invocation_type)?;
        let name = format_ident!("{}", name.file_name().unwrap().to_str().unwrap());
        stream.extend(quote! {
            mod #name {
                use super::*;
                #sub_stream
            }
        });
    }

    Ok(())
}

fn test_each(input: proc_macro::TokenStream, invocation_type: &Type) -> proc_macro::TokenStream {
    let parsed = parse_macro_input!(input as ForEachArgs);

    if !Path::new(&parsed.path.value()).is_dir() {
        abort_token_stream!(parsed.path.span(), "Given directory does not exist");
    }

    let mut tokens = TokenStream::new();

    let files = match Tree::new(parsed.path.value().as_ref(), !parsed.extensions.is_empty()) {
        Ok(files) => files,
        Err(e) => abort_token_stream!(parsed.path.span(), e),
    };

    if let Err(e) = generate_from_tree(&files, &parsed, &mut tokens, invocation_type) {
        abort_token_stream!(parsed.path.span(), e)
    }

    if let Some(module) = parsed.module {
        tokens = quote! {
            #[cfg(test)]
            mod #module {
                use super::*;
                #tokens
            }
        }
    }

    proc_macro::TokenStream::from(tokens)
}

/// Easily generate tests for files in a specified directory for comprehensive testing.
///
/// See crate level documentation for details.
#[proc_macro]
pub fn test_each_file(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    test_each(input, &Type::File)
}

/// Easily generate tests for paths in a specified directory for comprehensive testing.
///
/// See crate level documentation for details.
#[proc_macro]
pub fn test_each_path(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    test_each(input, &Type::Path)
}
