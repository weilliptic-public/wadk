extern crate proc_macro;
use contract::{
    impl_smart_contract_callback_macro, impl_smart_contract_constructor_macro,
    impl_smart_contract_macro, impl_smart_contract_mutate_macro, impl_smart_contract_query_macro,
    impl_smart_contract_xpod_macro,
};
use event::impl_event_macro;
use proc_macro::TokenStream;
use syn::{parse, parse_macro_input, DeriveInput, ImplItemFn, ItemImpl, ItemStruct, Meta};
use weil_type::impl_weil_type_derive;

use crate::contract::{
    impl_smart_contract_secured_macro, impl_smart_contract_tenancy_macro, QueryOpaqueKind,
    SecuredPurpose,
};

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

fn parse_required_secured_literal(attr: TokenStream) -> syn::Result<String> {
    if attr.is_empty() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[secured_user]/#[secured_admin] requires a literal group/subgroup argument, e.g. \
             #[secured_user(\"org.subgroup\")] — for multi-tenant parameter mode use \
             #[tenancy_user]/#[tenancy_admin] instead",
        ));
    }

    syn::parse::<syn::LitStr>(attr).map(|lit| lit.value())
}

/// Metadata mode only: `group`/`sub_group` are fixed at compile time from a
/// required literal (e.g. `#[secured_user("engg.weil")]`, split on `.` — last
/// segment is `group`, the rest joined by `_` is `sub_group`; no `.` means
/// org-level). Fixed per decorator invocation, so different methods on the
/// same contract can each pin a different org/subgroup.
///
/// For the multi-tenant case — one deployment serving many orgs, deciding
/// which on each call — use `#[tenancy_user]`/`#[tenancy_admin]` instead. The
/// two are separate mechanisms for separate situations, never combined on
/// one method.
///
/// See [`impl_smart_contract_secured_macro`].
#[proc_macro_attribute]
pub fn secured_user(attr: TokenStream, item: TokenStream) -> TokenStream {
    let literal = match parse_required_secured_literal(attr) {
        Ok(literal) => literal,
        Err(err) => return proc_macro::TokenStream::from(err.to_compile_error()),
    };

    let smart_contract_method = match parse::<ImplItemFn>(item) {
        Ok(syntax_tree) => syntax_tree,
        Err(err) => return proc_macro::TokenStream::from(err.to_compile_error()),
    };

    impl_smart_contract_secured_macro(SecuredPurpose::Execution, literal, smart_contract_method)
}

/// Requires `Management` only. Same literal-only convention as
/// [`secured_user`]. See [`impl_smart_contract_secured_macro`].
#[proc_macro_attribute]
pub fn secured_admin(attr: TokenStream, item: TokenStream) -> TokenStream {
    let literal = match parse_required_secured_literal(attr) {
        Ok(literal) => literal,
        Err(err) => return proc_macro::TokenStream::from(err.to_compile_error()),
    };

    let smart_contract_method = match parse::<ImplItemFn>(item) {
        Ok(syntax_tree) => syntax_tree,
        Err(err) => return proc_macro::TokenStream::from(err.to_compile_error()),
    };

    impl_smart_contract_secured_macro(SecuredPurpose::Management, literal, smart_contract_method)
}

fn check_no_tenancy_args(attr: TokenStream, name: &str) -> syn::Result<()> {
    if attr.is_empty() {
        Ok(())
    } else {
        Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("#[{}] takes no arguments", name),
        ))
    }
}

/// Gates a method on the caller holding `Execution` or `Management` purpose,
/// resolving `group`/`sub_group` from this call's own `SecuredOrg`/
/// `SecuredSubgroup` parameters rather than a compile-time literal — the
/// multi-tenant case (e.g. `codensics`), one deployment serving every org,
/// deciding which on each call.
///
/// The parameter is never trusted alone: it must match the caller's own
/// `Runtime::org()` claim exactly, or the call is denied before any Identity
/// lookup happens — a caller can act as the org/subgroup they actually signed
/// for, never one they merely pass as an argument.
///
/// Mutually exclusive with `#[secured_user]`/`#[secured_admin]` — a method
/// uses one mechanism or the other, never both.
#[proc_macro_attribute]
pub fn tenancy_user(attr: TokenStream, item: TokenStream) -> TokenStream {
    if let Err(err) = check_no_tenancy_args(attr, "tenancy_user") {
        return proc_macro::TokenStream::from(err.to_compile_error());
    }

    let smart_contract_method = match parse::<ImplItemFn>(item) {
        Ok(syntax_tree) => syntax_tree,
        Err(err) => return proc_macro::TokenStream::from(err.to_compile_error()),
    };

    impl_smart_contract_tenancy_macro(SecuredPurpose::Execution, smart_contract_method)
}

/// Requires `Management` only. Same parameter-mode resolution and
/// cross-check as [`tenancy_user`], just a stricter purpose check.
#[proc_macro_attribute]
pub fn tenancy_admin(attr: TokenStream, item: TokenStream) -> TokenStream {
    if let Err(err) = check_no_tenancy_args(attr, "tenancy_admin") {
        return proc_macro::TokenStream::from(err.to_compile_error());
    }

    let smart_contract_method = match parse::<ImplItemFn>(item) {
        Ok(syntax_tree) => syntax_tree,
        Err(err) => return proc_macro::TokenStream::from(err.to_compile_error()),
    };

    impl_smart_contract_tenancy_macro(SecuredPurpose::Management, smart_contract_method)
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
pub fn event(attr: TokenStream, item: TokenStream) -> TokenStream {
    let event_struct = match parse::<ItemStruct>(item) {
        Ok(syntax_tree) => syntax_tree,
        Err(err) => return proc_macro::TokenStream::from(err.to_compile_error()),
    };

    impl_event_macro(attr, event_struct)
}
