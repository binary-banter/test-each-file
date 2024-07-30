use std::path::Path;

#[proc_macro]
pub fn test_each_file(_input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    assert!(Path::new("./ROOT.txt").is_file());

    proc_macro::TokenStream::new()
}