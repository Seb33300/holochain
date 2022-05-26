use heck::ToSnakeCase;
use proc_macro_error::abort_call_site;
use syn::Fields;
use syn::Token;

pub fn to_snake_name(name: Option<String>, v_ident: &syn::Ident) -> String {
    match name {
        Some(s) => s,
        None => v_ident.to_string().to_snake_case(),
    }
}

pub fn ignore_enum_data(fields: &Fields) -> proc_macro2::TokenStream {
    match fields {
        syn::Fields::Named(_) => quote::quote! {{..}},
        syn::Fields::Unit => quote::quote! {},
        syn::Fields::Unnamed(_) => quote::quote! {(_)},
    }
}

pub fn get_unit_ident(attrs: &Vec<syn::Attribute>) -> syn::Ident {
    match darling::util::parse_attribute_to_meta_list(
        attrs
            .iter()
            .find(|a| {
                a.path
                    .segments
                    .last()
                    .map_or(false, |s| s.ident == "unit_enum")
            })
            .expect("Must have 'unit_enum' attribute"),
    ) {
        Ok(syn::MetaList { path, nested, .. }) if path.is_ident("unit_enum") => {
            match nested.first() {
                Some(syn::NestedMeta::Meta(syn::Meta::Path(path))) => path
                    .get_ident()
                    .expect("Failed to parse meta to ident")
                    .clone(),
                _ => todo!(),
            }
        }
        _ => todo!(),
    }
}

pub fn index_to_u8(index: usize) -> u8 {
    match u8::try_from(index) {
        Ok(i) => i,
        Err(_) => abort_call_site!("Can only have a maximum of 256 enum variants"),
    }
}