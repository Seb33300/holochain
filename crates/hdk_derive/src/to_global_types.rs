use proc_macro::TokenStream;
use proc_macro_error::abort;
use syn::parse_macro_input;
use syn::Item;
use syn::ItemEnum;
use syn::ItemStruct;

enum Category {
    Entries,
    Links,
}

pub fn build_entry(_args: TokenStream, input: TokenStream) -> TokenStream {
    build(Category::Entries, input)
}

pub fn build_link(_args: TokenStream, input: TokenStream) -> TokenStream {
    build(Category::Links, input)
}

fn build(category: Category, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Item);
    let ident = match &input {
        Item::Enum(ItemEnum { ident, .. }) | Item::Struct(ItemStruct { ident, .. }) => ident,
        r => {
            abort!(
                r,
                "The `to_global_types` macro can only be used on enums or structs."
            )
        }
    };
    let category = match category {
        Category::Entries => quote::quote! {.entries},
        Category::Links => quote::quote! {.links},
    };
    let output = quote::quote! {
        #input

        impl TryFrom<&#ident> for GlobalZomeTypeId {
            type Error = WasmError;

            fn try_from(value: &#ident) -> Result<Self, Self::Error> {
                zome_info()?
                    .zome_types
                    #category
                    .to_global_scope(value)
                    .ok_or_else(|| {
                        WasmError::Guest(format!(
                            "Value {:?} does not map to a global entry type for current scope.",
                            value
                        ))
                    })
            }
        }


        impl TryFrom<#ident> for GlobalZomeTypeId {
            type Error = WasmError;

            fn try_from(value: #ident) -> Result<Self, Self::Error> {
                Self::try_from(&value)
            }
        }
    };
    // let output = expander::Expander::new("to_global_entry_types")
    //     .fmt(expander::Edition::_2021)
    //     .verbose(true)
    //     // common way of gating this, by making it part of the default feature set
    //     .dry(false)
    //     .write_to(
    //         output.clone(),
    //         &std::path::Path::new("/home/freesig/holochain/holochain/"),
    //     )
    //     .unwrap_or_else(|e| {
    //         eprintln!("Failed to write to file: {:?}", e);
    //         output
    //     });
    output.into()
}
