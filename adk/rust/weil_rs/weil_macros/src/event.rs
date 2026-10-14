use proc_macro2::Literal;
use quote::{format_ident, quote};
use syn::{Error, Fields, ItemStruct, LitStr};

pub fn impl_event_macro(
    attr: proc_macro::TokenStream,
    input: ItemStruct,
) -> proc_macro::TokenStream {
    let publisher_address = match syn::parse::<LitStr>(attr) {
        Ok(address) => address,
        Err(err) => return err.to_compile_error().into(),
    };

    if !input.generics.params.is_empty() || input.generics.where_clause.is_some() {
        return Error::new_spanned(input.generics, "#[event] does not support generic structs")
            .to_compile_error()
            .into();
    }

    let Fields::Named(fields) = &input.fields else {
        return Error::new_spanned(input.ident, "#[event] requires a struct with named fields")
            .to_compile_error()
            .into();
    };

    let attrs = &input.attrs;
    let vis = &input.vis;
    let ident = &input.ident;
    let topic = Literal::string(&ident.to_string());
    let payload_ident = format_ident!("__{}Payload", ident);

    let mut original_fields = Vec::new();
    let mut field_names = Vec::new();
    let mut field_types = Vec::new();

    for field in fields.named.iter() {
        let Some(field_ident) = &field.ident else {
            return Error::new_spanned(field, "#[event] requires named fields")
                .to_compile_error()
                .into();
        };

        let field_attrs = &field.attrs;
        let field_vis = &field.vis;
        let field_ty = &field.ty;

        original_fields.push(quote! {
            #(#field_attrs)*
            #field_vis #field_ident: #field_ty
        });
        field_names.push(field_ident);
        field_types.push(field_ty);
    }

    quote! {
        #(#attrs)*
        #vis struct #ident {
            #(#original_fields,)*
            #[doc(hidden)]
            publisher_address: ::std::string::String,
        }

        impl #ident {
            pub const TOPIC: &'static str = #topic;
            pub const PUBLISHER_ADDRESS: &'static str = #publisher_address;

            pub fn new(#(#field_names: #field_types),*) -> ::std::result::Result<Self, ::std::string::String> {
                #[derive(::serde::Serialize)]
                struct CreateTopicArgs {
                    topic: ::std::string::String,
                    description: ::std::string::String,
                    retention_in_bytes: u64,
                }

                let create_topic_args = CreateTopicArgs {
                    topic: Self::TOPIC.to_string(),
                    description: Self::TOPIC.to_string(),
                    retention_in_bytes: ::weil_rs::event::DEFAULT_RETENTION_BYTES,
                };
                let create_topic_payload = ::serde_json::to_string(&create_topic_args)
                    .map_err(|err| err.to_string())?;

                let _ = ::weil_rs::runtime::Runtime::call_contract::<()>(
                    Self::PUBLISHER_ADDRESS.to_string(),
                    "create_topic".to_string(),
                    Some(create_topic_payload),
                );

                Ok(Self {
                    #(#field_names,)*
                    publisher_address: Self::PUBLISHER_ADDRESS.to_string(),
                })
            }

            pub fn publisher_address(&self) -> &str {
                &self.publisher_address
            }

            pub fn emit(&self) -> ::std::result::Result<::serde_json::Value, ::std::string::String> {
                #[derive(::serde::Serialize)]
                struct #payload_ident<'a> {
                    #(#field_names: &'a #field_types,)*
                }

                #[derive(::serde::Serialize)]
                struct PublishArgs {
                    topic: ::std::string::String,
                    payload: ::std::string::String,
                }

                let payload = #payload_ident {
                    #(#field_names: &self.#field_names,)*
                };
                let serialized_payload = ::serde_json::to_string(&payload)
                    .map_err(|err| err.to_string())?;
                let publish_args = PublishArgs {
                    topic: Self::TOPIC.to_string(),
                    payload: serialized_payload,
                };
                let publish_payload = ::serde_json::to_string(&publish_args)
                    .map_err(|err| err.to_string())?;

                let receipt = ::weil_rs::runtime::Runtime::call_contract::<::serde_json::Value>(
                    self.publisher_address.clone(),
                    "publish".to_string(),
                    Some(publish_payload),
                )
                .unwrap_or(::serde_json::Value::Null);

                Ok(receipt)
            }
        }

        macro_rules! emit {
            ($event:ident($($field:ident $(: $value:expr)?),* $(,)?)) => {{
                match $event::new($(emit!(@value $field $($value)?)),*) {
                    Ok(event) => event.emit(),
                    Err(err) => Err(err),
                }
            }};
            (@value $field:ident $value:expr) => {
                $value
            };
            (@value $field:ident) => {
                $field
            };
        }
    }
    .into()
}
