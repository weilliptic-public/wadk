use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Data, DataEnum, DataStruct, DeriveInput, Error, Ident, Type};

pub fn impl_weil_type_for_struct<'a>(struct_data: &'a DataStruct) -> Vec<&'a Type> {
    let mut field_types = vec![];

    for field in struct_data.fields.iter() {
        field_types.push(&field.ty)
    }

    field_types
}

pub fn impl_weil_type_for_enum<'a>(enum_data: &'a DataEnum) -> Vec<&'a Type> {
    let mut field_types: Vec<&Type> = vec![];

    for variant in enum_data.variants.iter() {
        for field in variant.fields.iter() {
            field_types.push(&field.ty);
        }
    }

    field_types
}

pub fn impl_weil_type_derive(input: DeriveInput) -> proc_macro::TokenStream {
    let name = &input.ident;
    let data = &input.data;

    let field_types = match data {
        Data::Struct(struct_data) => impl_weil_type_for_struct(struct_data),
        Data::Enum(enum_data) => impl_weil_type_for_enum(enum_data),
        Data::Union(_) => {
            return Error::new_spanned(name, "`WeilType` derive macro cannot be used on Union")
                .to_compile_error()
                .into();
        }
    };

    let assets: Vec<TokenStream> = field_types
        .iter()
        .map(|ty| {
            quote! {
                assert_weil_type_bound_for_field_type::<#ty>();
            }
        })
        .collect();

    let assert_func_name = Ident::new(
        &format!(
            "assert_weil_type_bound_for_{}_field_type",
            name.to_string().to_lowercase()
        ),
        Span::call_site(),
    );

    let derive_impls: TokenStream = quote! {
        fn #assert_func_name() {
            fn assert_weil_type_bound_for_field_type<T>()
            where
            T: weil_rs::traits::WeilType,
            {}

            #(#assets)*
        }

        impl weil_rs::traits::WeilType for #name {}
    }
    .into();

    quote! {
        #derive_impls
    }
    .into()
}
