#![doc = include_str!("../README.md")]
use proc_macro2::{Ident, TokenStream};
use proc_macro_error::{abort, abort_call_site, proc_macro_error};
use quote::{format_ident, quote};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{bracketed, parse_macro_input, Expr, LitStr, Token};
use unicode_ident::{is_xid_continue, is_xid_start};

struct TestEachArgs {
    path: LitStr,
    module: Option<Ident>,
    function: Expr,
    extensions: Vec<String>,
}

impl Parse for TestEachArgs {
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
    fn new(base: &Path, extensions: &[String]) -> Self {
        let mut tree = Self::default();
        for entry in base.read_dir().unwrap() {
            let mut entry = entry.unwrap().path();
            if entry.is_file() {
                if !extensions.is_empty() {
                    // Ignore file if it does not have one extension.
                    let Some(extension) = entry.extension() else {
                        continue;
                    };
                    // Ignore the file if the extension is not contained in the provided extensions.
                    if !extensions
                        .iter()
                        .any(|test_extension| test_extension == extension.to_str().unwrap())
                    {
                        continue;
                    }
                    // Trim extension.
                    entry.set_extension("");
                }
                tree.here.insert(entry);
            } else if entry.is_dir() {
                tree.children.insert(
                    entry.as_path().to_path_buf(),
                    Self::new(entry.as_path(), extensions),
                );
            } else {
                abort_call_site!(format!("Unsupported path: {:#?}.", entry))
            }
        }
        tree
    }
}

enum Type {
    File,
    Path,
}

/// Sanitize a string so that it can be a valid identifier.
/// Replaces invalid characters with underscores
fn sanitize_ident(input: &str) -> Ident {
    let name: String = input
        .chars()
        .map(|c| if is_xid_continue(c) { c } else { '_' })
        .collect();

    if !is_xid_start(name.chars().next().expect("Name is not empty")) {
        format_ident!("test_{name}")
    } else {
        format_ident!("{name}")
    }
}

/// Given a starting name and a set of taken names, generate a new name that is unique in the `taken_names` set
fn generate_name(starting_name: Ident, taken_names: &mut HashSet<Ident>) -> Ident {
    if taken_names.insert(starting_name.clone()) {
        return starting_name;
    }

    for i in 2.. {
        let new_name = format_ident!("{starting_name}_{i}");
        if taken_names.insert(new_name.clone()) {
            return new_name;
        }
    }

    unreachable!()
}

fn generate_from_tree(
    tree: &Tree,
    parsed: &TestEachArgs,
    stream: &mut TokenStream,
    invocation_type: &Type,
) {
    let mut taken_names_folders = HashSet::new();
    for (name, directory) in tree.children.iter() {
        let file_name = name.file_name().unwrap().to_str().unwrap();
        let file_name = sanitize_ident(file_name);
        let file_name = generate_name(file_name, &mut taken_names_folders);

        let mut sub_stream = TokenStream::new();
        generate_from_tree(directory, parsed, &mut sub_stream, invocation_type);
        stream.extend(quote! {
            mod #file_name {
                use super::*;
                #sub_stream
            }
        });
    }

    let mut taken_names_files = HashSet::new();
    for file in tree.here.iter() {
        let file_name = file.file_stem().unwrap().to_str().unwrap();
        let file_name = sanitize_ident(file_name);
        let file_name = generate_name(file_name, &mut taken_names_files);

        let function = &parsed.function;

        let arguments: TokenStream = if parsed.extensions.is_empty() {
            let input = file.canonicalize().unwrap();
            let input = input.to_str().unwrap();

            match invocation_type {
                Type::File => quote!(include_str!(#input)),
                Type::Path => quote!(std::path::Path::new(#input)),
            }
        } else {
            let mut arguments = TokenStream::new();

            for extension in &parsed.extensions {
                let input = file.with_extension(extension).canonicalize().unwrap();
                if !input.exists() {
                    abort_call_site!(format!(
                        "Expected file {:?} with extension {}, but it does not exist.",
                        file, extension
                    ))
                }
                let input = input.to_str().unwrap();

                arguments.extend(match invocation_type {
                    Type::File => quote!(include_str!(#input),),
                    Type::Path => quote!(std::path::Path::new(#input),),
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
}

fn test_each(input: proc_macro::TokenStream, invocation_type: &Type) -> proc_macro::TokenStream {
    let parsed = parse_macro_input!(input as TestEachArgs);

    if !Path::new(&parsed.path.value()).is_dir() {
        abort!(parsed.path.span(), "Given directory does not exist");
    }

    let mut tokens = TokenStream::new();
    let files = Tree::new(parsed.path.value().as_ref(), &parsed.extensions);
    generate_from_tree(&files, &parsed, &mut tokens, invocation_type);

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
#[proc_macro_error]
pub fn test_each_file(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    test_each(input, &Type::File)
}

/// Easily generate tests for paths in a specified directory for comprehensive testing.
///
/// See crate level documentation for details.
#[proc_macro]
#[proc_macro_error]
pub fn test_each_path(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    test_each(input, &Type::Path)
}
