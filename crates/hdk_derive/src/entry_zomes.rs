use proc_macro::TokenStream;
use syn::parse_macro_input;
use syn::Item;
use syn::ItemEnum;

use crate::util::index_to_u8;

pub fn build(_attrs: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Item);

    let (ident, variants) = match &input {
        Item::Enum(ItemEnum {
            ident, variants, ..
        }) => (ident, variants),
        _ => todo!(),
    };

    let index_to_variant: proc_macro2::TokenStream = variants
        .iter()
        .enumerate()
        .map(|(i, v)|(index_to_u8(i), v))
        .map(|(i, syn::Variant { ident: v_ident, fields, .. })| {
            let ty = &fields.iter().next().unwrap().ty;
            quote::quote! {
                if ((<#ident as EnumVariantLen<#i>>::ENUM_VARIANT_START)..(<#ident as EnumVariantLen<#i>>::ENUM_VARIANT_LEN)).contains(&value) {
                    return Ok(Self::#v_ident(#ty::try_from_local_type::<LocalZomeTypeId>(LocalZomeTypeId(value), entry)?)); 
                }
            }
        })
        .collect();

    let try_into_entry: proc_macro2::TokenStream = variants
        .into_iter()
        .map(
            |syn::Variant {
                 ident: v_ident,
                 fields,
                 ..
             }| {
                match fields {
                    syn::Fields::Named(_) => todo!(),
                    syn::Fields::Unit => todo!(),
                    syn::Fields::Unnamed(_) => {
                        // TODO: Error if fields is longer then one.
                        quote::quote! {#ident::#v_ident (v) => Entry::try_from(v),}
                    }
                }
            },
        )
        .collect();

    let output = quote::quote! {
        #[hdk_to_global_entry_types]
        #[hdk_to_local_types(nested = true)]
        #[derive(Debug)]
        #input

        impl TryFrom<&#ident> for Entry {
            type Error = WasmError;

            fn try_from(value: &#ident) -> Result<Self, Self::Error> {
                match value {
                    #try_into_entry
                }
            }
        }

        impl TryFrom<#ident> for Entry {
            type Error = WasmError;

            fn try_from(value: #ident) -> Result<Self, Self::Error> {
                Entry::try_from(&value)
            }
        }

        impl TryFrom<&#ident> for EntryDefIndex {
            type Error = WasmError;

            fn try_from(value: &#ident) -> Result<Self, Self::Error> {
                Ok(Self(GlobalZomeTypeId::try_from(value)?.0))
            }
        }

        impl TryFrom<#ident> for EntryDefIndex {
            type Error = WasmError;

            fn try_from(value: #ident) -> Result<Self, Self::Error> {
                Self::try_from(&value)
            }
        }

        impl TryFrom<&&#ident> for EntryDefIndex {
            type Error = WasmError;

            fn try_from(value: &&#ident) -> Result<Self, Self::Error> {
                Self::try_from(*value)
            }
        }

        impl EntryTypesHelper for #ident {
            fn try_from_local_type<I>(type_index: I, entry: &Entry) -> Result<Self, WasmError>
            where
                LocalZomeTypeId: From<I>,
            {

                let value = LocalZomeTypeId::from(type_index).0;
                #index_to_variant
                todo!()
            }
            fn try_from_global_type<I>(type_index: I, entry: &Entry) -> Result<Self, WasmError>
            where
                GlobalZomeTypeId: From<I>,
            {
                let index: GlobalZomeTypeId = type_index.into();
                match zome_info()?.zome_types.entries.to_local_scope(index) {
                    Some(local_index) => Self::try_from_local_type(local_index, &entry),
                    _ => Err(WasmError::Guest(format!(
                        "global index {} does not map to any local scope for this zome",
                        index.0
                    ))),
                }
            }
        }

    };
    // let output = expander::Expander::new("entry_zomes")
    //     .fmt(expander::Edition::_2021)
    //     .verbose(true)
    //     // common way of gating this, by making it part of the default feature set
    //     .dry(false)
    //     .write_to(output.clone(), &std::path::Path::new("/home/freesig/holochain/holochain/")).unwrap_or_else(|e| {
    //         eprintln!("Failed to write to file: {:?}", e);
    //         output
    //     });
    output.into()
}
