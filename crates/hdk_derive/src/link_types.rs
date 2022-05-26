use proc_macro::TokenStream;
use syn::parse_macro_input;
use syn::Item;
use syn::ItemEnum;

pub fn build(_attrs: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Item);
    let (ident, variants) = match &input {
        Item::Enum(ItemEnum {
            ident, variants, ..
        }) => (ident, variants),
        _ => todo!(),
    };
    let units: proc_macro2::TokenStream = variants
        .iter()
        .map(|syn::Variant { ident, .. }| quote::quote! {#ident,})
        .collect();

    let output = quote::quote! {
        #[hdk_to_global_link_types]
        #[hdk_to_local_types]
        #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
        #input

        impl TryFrom<#ident> for LinkType {
            type Error = WasmError;

            fn try_from(value: #ident) -> Result<Self, Self::Error> {
                Ok(Self(GlobalZomeTypeId::try_from(value)?.0))
            }
        }

        impl TryFrom<&#ident> for LinkType {
            type Error = WasmError;

            fn try_from(value: &#ident) -> Result<Self, Self::Error> {
                Ok(Self(GlobalZomeTypeId::try_from(value)?.0))
            }
        }

        impl TryFrom<#ident> for LinkTypeRange {
            type Error = WasmError;

            fn try_from(value: #ident) -> Result<Self, Self::Error> {
                let lt: LinkType = value.try_into()?;
                Ok(lt.into())
            }
        }

        impl TryFrom<&#ident> for LinkTypeRange {
            type Error = WasmError;

            fn try_from(value: &#ident) -> Result<Self, Self::Error> {
                let lt: LinkType = value.try_into()?;
                Ok(lt.into())
            }
        }

        impl TryFrom<#ident> for LinkTypeRanges {
            type Error = WasmError;

            fn try_from(value: #ident) -> Result<Self, Self::Error> {
                let lt: LinkType = value.try_into()?;
                Ok(Self(vec![lt.into()]))
            }
        }

        impl TryFrom<&#ident> for LinkTypeRanges {
            type Error = WasmError;

            fn try_from(value: &#ident) -> Result<Self, Self::Error> {
                let lt: LinkType = value.try_into()?;
                Ok(Self(vec![lt.into()]))
            }
        }

        impl LinkTypesHelper<{ #ident::len() }, { #ident::len() as usize }> for #ident {
            fn iter() -> core::array::IntoIter<Self, { #ident::len() as usize }> {
                use #ident::*;
                [#units].into_iter()
            }
        }

        impl TryFrom<LocalZomeTypeId> for #ident {
            type Error = WasmError;

            fn try_from(value: LocalZomeTypeId) -> Result<Self, Self::Error> {
                Self::iter()
                    .find(|u| LocalZomeTypeId::from(*u) == value)
                    .ok_or_else(|| {
                        WasmError::Guest(format!(
                            "local index {:?} does not match any variant of {}",
                            value, stringify!(#ident)
                        ))
                    })
            }
        }

        impl TryFrom<&LocalZomeTypeId> for #ident {
            type Error = WasmError;

            fn try_from(value: &LocalZomeTypeId) -> Result<Self, Self::Error> {
                Self::try_from(*value)
            }
        }

        impl TryFrom<GlobalZomeTypeId> for #ident {
            type Error = WasmError;

            fn try_from(index: GlobalZomeTypeId) -> Result<Self, Self::Error> {
                match zome_info()?.zome_types.links.to_local_scope(index) {
                    Some(local_index) => Self::try_from(local_index),
                    _ => Err(WasmError::Guest(format!(
                        "global index {:?} does not map to any local scope for this zome",
                        index
                    ))),
                }
            }
        }

        impl TryFrom<&GlobalZomeTypeId> for #ident {
            type Error = WasmError;

            fn try_from(index: &GlobalZomeTypeId) -> Result<Self, Self::Error> {
                Self::try_from(*index)
            }
        }

        impl TryFrom<LinkType> for #ident {
            type Error = WasmError;
            fn try_from(index: LinkType) -> Result<Self, Self::Error> {
                let index: GlobalZomeTypeId = index.into();
                Self::try_from(index)
            }
        }

        impl TryFrom<&LinkType> for #ident {
            type Error = WasmError;
            fn try_from(index: &LinkType) -> Result<Self, Self::Error> {
                Self::try_from(*index)
            }
        }

    };
    output.into()
}
