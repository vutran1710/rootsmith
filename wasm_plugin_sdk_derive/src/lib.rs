use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;
use syn::Attribute;
use syn::Data;
use syn::DataStruct;
use syn::DeriveInput;
use syn::Fields;
use syn::Meta;

/*
 * Expected parser code
 * use sdk::*;
 *
 * fn decode(input: UpstreamData) -> Result<Option<Record>> {
 *   match input {
 *    UpstreamData::Json(json_value) => {
  *       Some(Record {
 *              namespace: get_field::<String>(json_value, "namespace")?,
 *             key: get_field::<String>(json_value, "key")?,
 *       })
 *     _ => Ok(None),
 *   }
 *
 */

/// Derive macro for DecodeFromEnvelope trait
/// Supports #[decode(from = "json")] attribute
#[proc_macro_derive(DecodeFromEnvelope, attributes(decode))]
pub fn decode_from_envelope_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // Check for #[decode(from = "json")] attribute
    // For now, we'll always generate JSON parsing (since that's what the user wants)
    let decode_from_json = true;

    // Extract struct fields
    let fields = match &input.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => &fields.named,
        _ => {
            return syn::Error::new_spanned(
                &input,
                "DecodeFromEnvelope can only be derived for structs with named fields",
            )
            .to_compile_error()
            .into();
        }
    };

    // Generate field extraction code
    let field_extractions: Vec<_> = fields.iter().map(|field| {
        let field_name = &field.ident;
        let field_type = &field.ty;

        // Determine extraction method based on field type
        // Check if it's sdk::String or String-like by examining the type
        let type_str = quote!(#field_type).to_string();
        let is_string = type_str.contains("String");
        let is_u64 = type_str.trim() == "u64";

        if is_u64 {
            quote! {
                #field_name: crate::infra::extract_json_u64_field(input_str, stringify!(#field_name))?
            }
        } else {
            // Default: string extraction (for sdk::String and other types)
            quote! {
                #field_name: crate::infra::extract_json_string_field(input_str, stringify!(#field_name))?
            }
        }
    }).collect();

    // Generate the implementation
    let expanded = quote! {
        impl crate::infra::sdk::DecodeFromEnvelope for #name {
            fn decode_from_json(input: &[u8]) -> Option<Self> {
                unsafe { crate::infra::STRING_OFFSET = 0; }
                let input_str = core::str::from_utf8(input).ok()?;

                Some(#name {
                    #(#field_extractions),*
                })
            }
        }
    };

    TokenStream::from(expanded)
}
