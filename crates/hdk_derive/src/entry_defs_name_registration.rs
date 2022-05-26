use darling::FromMeta;
use proc_macro::TokenStream;
use proc_macro_error::abort;
use syn::parse_macro_input;
use syn::AttributeArgs;
use syn::Item;
use syn::ItemEnum;

use crate::util::get_unit_ident;

#[derive(Debug, FromMeta)]
pub struct MacroArgs {
    #[darling(default)]
    skip_hdk_extern: bool,
}

pub fn build(attrs: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Item);
    let attr_args = parse_macro_input!(attrs as AttributeArgs);

    let (ident, variants, attrs) = match &input {
        Item::Enum(ItemEnum {
            ident,
            variants,
            attrs,
            ..
        }) => (ident, variants, attrs),
        _ => todo!(),
    };

    let unit_ident = get_unit_ident(attrs);

    let units_to_full: proc_macro2::TokenStream = variants.iter()
        .map(|syn::Variant{ident: v_ident, .. }| {
            quote::quote! {
                #unit_ident::#v_ident => Ok(Self::#v_ident(entry.try_into()?)),
            }
        }).collect();

    let skip_hdk_extern = match MacroArgs::from_list(&attr_args) {
        Ok(a) => a.skip_hdk_extern,
        Err(e) => abort!(ident, "not sure {:?}", e),
    };

    let hdk_extern = if skip_hdk_extern {
        quote::quote! {}
    } else {
        quote::quote! {#[hdk_extern]}
    };

    let output = quote::quote! {
        #[derive(EntryDefRegistration, UnitEnum)]
        #[unit_attrs(forward(hdk_to_local_types, hdk_to_global_entry_types))]
        #input

        #hdk_extern
        pub fn entry_defs(_: ()) -> ExternResult<EntryDefsCallbackResult> {
            let defs: Vec<EntryDef> = #ident::ENTRY_DEFS
                    .iter()
                    .map(|a| EntryDef::from(a.clone()))
                    .collect();
            Ok(EntryDefsCallbackResult::from(defs))
        }

        impl TryFrom<&#ident> for GlobalZomeTypeId {
            type Error = WasmError;

            fn try_from(value: &#ident) -> Result<Self, Self::Error> {
                Self::try_from(value.to_unit())
            }
        }

        impl TryFrom<#ident> for GlobalZomeTypeId {
            type Error = WasmError;

            fn try_from(value: #ident) -> Result<Self, Self::Error> {
                Self::try_from(&value)
            }
        }

        impl TryFrom<&#ident> for EntryDefIndex {
            type Error = WasmError;

            fn try_from(value: &#ident) -> Result<Self, Self::Error> {
                Ok(Self(GlobalZomeTypeId::try_from(value.to_unit())?.0))
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

        impl From<&#ident> for LocalZomeTypeId {
            fn from(t: &#ident) -> Self {
                Self::from(t.to_unit())
            }
        }

        impl From<#ident> for LocalZomeTypeId {
            fn from(t: #ident) -> Self {
                Self::from(&t)
            }
        }

        impl TryFrom<LocalZomeTypeId> for #unit_ident {
            type Error = WasmError;

            fn try_from(value: LocalZomeTypeId) -> Result<Self, Self::Error> {
                Self::iter()
                    .find(|u| LocalZomeTypeId::from(*u) == value)
                    .ok_or_else(|| {
                        WasmError::Guest(format!(
                            "local index {} does not match any variant of {}",
                            value.0, stringify!(#unit_ident)
                        ))
                    })
            }
        }

        impl EntryTypesHelper for #ident {
            fn try_from_local_type<I>(type_index: I, entry: &Entry) -> Result<Self, WasmError>
            where
                LocalZomeTypeId: From<I>,
            {
                match <#ident as UnitEnum>::Unit::try_from(LocalZomeTypeId::from(type_index))? {
                    #units_to_full
                }
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

        impl EnumLen<{<#ident as UnitEnum>::Unit::ENUM_LEN}> for #ident {}
    };
    // eprintln!("{}", &output);
    // let output = expander::Expander::new("entry_defs_name_registration")
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
