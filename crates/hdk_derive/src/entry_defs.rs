use proc_macro::TokenStream;
use syn::parse_macro_input;
use syn::Item;

pub fn build(attrs: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Item);
    let attr_args: proc_macro2::TokenStream = attrs.into();

    let output = quote::quote! {
        #[hdk_derive::entry_defs_name_registration(#attr_args)]
        #[hdk_derive::entry_defs_conversions]
        #[derive(Debug)]
        #input
    };
    // let output = expander::Expander::new("entry_defs_expand")
    //     .fmt(expander::Edition::_2021)
    //     .verbose(true)
    //     // common way of gating this, by making it part of the default feature set
    //     .dry(false)
    //     .write_to_out_dir(output.clone()).unwrap_or_else(|e| {
    //         eprintln!("Failed to write to file: {:?}", e);
    //         output
    //     });
    output.into()
}
