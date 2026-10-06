extern crate proc_macro;
use contract::{
    impl_smart_contract_callback_macro, impl_smart_contract_constructor_macro,
    impl_smart_contract_macro, impl_smart_contract_mutate_macro, impl_smart_contract_query_macro,
    impl_smart_contract_xpod_macro,
};
use event::{check_trait_item_fn, impl_event_macro};
use proc_macro::TokenStream;
use syn::{parse, parse_macro_input, DeriveInput, ImplItemFn, ItemImpl, Meta, TraitItemFn};
use weil_type::impl_weil_type_derive;

use crate::contract::{impl_smart_contract_secured_macro, QueryOpaqueKind, SecuredPurpose};

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

/// Gates a method on the caller holding `Execution` or `Management` purpose
/// in their organization's Identity applet.
///
/// Takes no arguments: the organization is resolved at runtime from
/// `Runtime::org()` — the org the calling wallet signed the transaction under
/// — so no `org` parameter appears in the method signature or the WIDL, and
/// one applet can serve many orgs, deciding per call.
///
/// That org is authenticated but self-declared, so it only selects *which*
/// Identity applet to interrogate; membership itself is proven by the purpose
/// check. A caller claiming an org or subgroup they do not belong to is
/// denied. See [`impl_smart_contract_secured_macro`].
#[proc_macro_attribute]
pub fn secured_user(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return proc_macro::TokenStream::from(
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "#[secured_user] takes no arguments",
            )
            .to_compile_error(),
        );
    }

    let smart_contract_method = match parse::<ImplItemFn>(item) {
        Ok(syntax_tree) => syntax_tree,
        Err(err) => return proc_macro::TokenStream::from(err.to_compile_error()),
    };

    impl_smart_contract_secured_macro(SecuredPurpose::Execution, smart_contract_method)
}

/// Requires `Management` only. Takes no arguments — same `Runtime::org()`
/// based org resolution as [`secured_user`], just a stricter purpose check.
/// See [`impl_smart_contract_secured_macro`].
#[proc_macro_attribute]
pub fn secured_admin(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return proc_macro::TokenStream::from(
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "#[secured_admin] takes no arguments",
            )
            .to_compile_error(),
        );
    }

    let smart_contract_method = match parse::<ImplItemFn>(item) {
        Ok(syntax_tree) => syntax_tree,
        Err(err) => return proc_macro::TokenStream::from(err.to_compile_error()),
    };

    impl_smart_contract_secured_macro(SecuredPurpose::Management, smart_contract_method)
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
