use quote::quote;
use syn::{parse, Block, ItemFn, TraitItemFn};

pub fn check_trait_item_fn(fn_decl: &TraitItemFn) -> Result<(), proc_macro::TokenStream> {
    Ok(())
}

pub fn impl_event_macro(fn_decl: TraitItemFn) -> proc_macro::TokenStream {
    if let Err(err) = check_trait_item_fn(&fn_decl) {
        return err;
    }

    let block = parse::<Block>(
        quote! {
            {
                println!("Hello, Yamini");
            }
        }
        .into(),
    )
    .unwrap();

    let expanded_fn = ItemFn {
        attrs: fn_decl.attrs,
        vis: syn::Visibility::Inherited,
        sig: fn_decl.sig,
        block: Box::new(block),
    };

    quote! {
        #expanded_fn
    }
    .into()
}
