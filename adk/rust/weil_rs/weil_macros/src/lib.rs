extern crate proc_macro;
use contract::{
    impl_smart_contract_callback_macro, impl_smart_contract_constructor_macro,
    impl_smart_contract_macro, impl_smart_contract_mutate_macro, impl_smart_contract_query_macro,
    impl_smart_contract_xpod_macro,
};
use event::{check_trait_item_fn, impl_event_macro};
use proc_macro::TokenStream;
use syn::{
    parse, parse_macro_input, DeriveInput, Ident, ImplItemFn, ItemImpl, LitStr, Meta, TraitItemFn,
};
use weil_type::impl_weil_type_derive;

use crate::contract::{impl_smart_contract_secured_macro, QueryOpaqueKind};

mod contract;
mod event;
mod weil_type;

#[proc_macro_derive(WeilType)]
pub fn weil_type_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    impl_weil_type_derive(input)
}

#[proc_macro_attribute]
pub fn smart_contract(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let impl_smart_contract = match parse::<ItemImpl>(item) {
        Ok(syntax_tree) => syntax_tree,
        Err(err) => return proc_macro::TokenStream::from(err.to_compile_error()),
    };

    impl_smart_contract_macro(impl_smart_contract)
}

#[proc_macro_attribute]
pub fn mutate(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let smart_contract_mutate_method = match parse::<ImplItemFn>(item) {
        Ok(syntax_tree) => syntax_tree,
        Err(err) => return proc_macro::TokenStream::from(err.to_compile_error()),
    };

    impl_smart_contract_mutate_macro(smart_contract_mutate_method)
}

#[proc_macro_attribute]
pub fn query(attr: TokenStream, item: TokenStream) -> TokenStream {
    let smart_contract_query_method = match parse::<ImplItemFn>(item) {
        Ok(syntax_tree) => syntax_tree,
        Err(err) => return proc_macro::TokenStream::from(err.to_compile_error()),
    };

    let mut query_opaque_kind = QueryOpaqueKind::Ordinary;

    let query_parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("stream") {
            query_opaque_kind = QueryOpaqueKind::Stream;

            return Ok(());
        } else if meta.path.is_ident("plottable") {
            query_opaque_kind = QueryOpaqueKind::Plottable;

            return Ok(());
        }

        Ok(())
    });

    parse_macro_input!(attr with query_parser);

    impl_smart_contract_query_macro(smart_contract_query_method, query_opaque_kind)
}

#[proc_macro_attribute]
pub fn secured(attr: TokenStream, item: TokenStream) -> TokenStream {
    let arg = match parse::<LitStr>(attr) {
        Ok(syntax_tree) => syntax_tree,
        Err(err) => return proc_macro::TokenStream::from(err.to_compile_error()),
    };

    //let arg_str = arg.to_string();

    let smart_contract_method = match parse::<ImplItemFn>(item) {
        Ok(syntax_tree) => syntax_tree,
        Err(err) => return proc_macro::TokenStream::from(err.to_compile_error()),
    };

    impl_smart_contract_secured_macro(arg.value(), smart_contract_method)
}

#[proc_macro_attribute]
pub fn xpod(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let smart_contract_query_method = match parse::<ImplItemFn>(item) {
        Ok(syntax_tree) => syntax_tree,
        Err(err) => return proc_macro::TokenStream::from(err.to_compile_error()),
    };

    impl_smart_contract_xpod_macro(smart_contract_query_method)
}

#[proc_macro_attribute]
pub fn callback(attr: TokenStream, item: TokenStream) -> TokenStream {
    let smart_contract_query_method = match parse::<ImplItemFn>(item) {
        Ok(syntax_tree) => syntax_tree,
        Err(err) => return proc_macro::TokenStream::from(err.to_compile_error()),
    };

    let meta = match parse::<Meta>(attr) {
        Ok(attr) => attr,
        Err(err) => return proc_macro::TokenStream::from(err.to_compile_error()),
    };

    let Meta::Path(path) = &meta else {
        return proc_macro::TokenStream::from(
            syn::Error::new_spanned(meta, &format!("invalid attribute")).to_compile_error(),
        );
    };

    let Some(path) = path.get_ident() else {
        return proc_macro::TokenStream::from(
            syn::Error::new_spanned(
                meta,
                &format!("invalid assosiated function name in callback macro"),
            )
            .to_compile_error(),
        );
    };

    impl_smart_contract_callback_macro(smart_contract_query_method, path.to_string())
}

#[proc_macro_attribute]
pub fn constructor(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let smart_contract_constructor = match parse::<ImplItemFn>(item) {
        Ok(syntax_tree) => syntax_tree,
        Err(err) => return proc_macro::TokenStream::from(err.to_compile_error()),
    };

    impl_smart_contract_constructor_macro(smart_contract_constructor)
}

#[proc_macro_attribute]
pub fn event(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let fn_decl = match parse::<TraitItemFn>(item) {
        Ok(syntax_tree) => syntax_tree,
        Err(err) => return proc_macro::TokenStream::from(err.to_compile_error()),
    };

    if let Err(err) = check_trait_item_fn(&fn_decl) {
        return err;
    }

    impl_event_macro(fn_decl)
}
