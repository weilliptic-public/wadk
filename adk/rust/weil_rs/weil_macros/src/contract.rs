use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::quote;
use syn::{
    parse,
    punctuated::Punctuated,
    token::{Comma, Dot, Paren},
    Block, Expr, ExprCall, ExprField, ExprMethodCall, ExprPath, FnArg, GenericArgument, ImplItem,
    ImplItemFn, ItemImpl, Member, Meta, Pat, Path, PathArguments, PathSegment, ReturnType,
    Signature, Token, Type, TypePath,
};

pub enum QueryOpaqueKind {
    Stream,
    Plottable,
    Ordinary,
}

impl QueryOpaqueKind {
    fn is_ordinary(&self) -> bool {
        matches!(self, QueryOpaqueKind::Ordinary)
    }
}

#[derive(Debug, Clone, Copy)]
enum SmartContractMethodAnnotationKind {
    Mutation,
    Query,
    QueryStream,
    QueryPlottable,
    Constructor(bool), // is_async
}

impl ToString for SmartContractMethodAnnotationKind {
    fn to_string(&self) -> String {
        match self {
            SmartContractMethodAnnotationKind::Mutation => "mutate".to_string(),
            SmartContractMethodAnnotationKind::Query => "query".to_string(),
            SmartContractMethodAnnotationKind::QueryStream => "query_stream".to_string(),
            SmartContractMethodAnnotationKind::QueryPlottable => "query_plottable".to_string(),
            SmartContractMethodAnnotationKind::Constructor(_) => "mutate".to_string(),
        }
    }
}

fn snake_to_camel_case(snake_case: &str) -> String {
    let mut camel_case = String::new();
    let mut capitalize_next = true;

    for c in snake_case.chars() {
        if c == '_' {
            capitalize_next = true;
        } else {
            if capitalize_next {
                camel_case.push(c.to_ascii_uppercase());
                capitalize_next = false;
            } else {
                camel_case.push(c);
            }
        }
    }

    camel_case
}

fn is_eq(ty: &Type, other: &str) -> bool {
    let Type::Path(ty) = ty else { return false };

    let Some(path) = ty.path.get_ident() else {
        return false;
    };

    path.to_string() == other
}

fn ok_err_ty_from_result(ty: &Type) -> Option<(&Type, &Type)> {
    let Type::Path(ty) = ty else {
        return None;
    };

    let Some(ty) = ty.path.segments.first() else {
        return None;
    };

    let ty_name = ty.ident.to_string();

    if ty_name != "Result" {
        return None;
    }

    let PathArguments::AngleBracketed(path) = &ty.arguments else {
        return None;
    };

    let args = &path.args;

    if args.len() != 2 {
        return None;
    }

    let GenericArgument::Type(ok_ty) = args.first().unwrap() else {
        return None;
    };

    let GenericArgument::Type(err_ty) = args.last().unwrap() else {
        return None;
    };

    Some((ok_ty, err_ty))
}

fn process_method_macro(meta: &Meta) -> Option<(String, Option<String>)> {
    let mut meta_arg: Option<String> = None;

    let path = match meta {
        Meta::Path(path) => path,
        Meta::List(list) => {
            match syn::parse2::<Ident>(list.tokens.clone()) {
                Ok(val) => meta_arg = Some(val.to_string()),
                Err(_) => return None,
            }

            &list.path
        }
        Meta::NameValue(_) => return None,
    };

    if path.segments.len() != 1 {
        return None;
    }

    let macro_name_path_seg = path.segments.first().unwrap();

    if !macro_name_path_seg.arguments.is_none() {
        return None;
    }

    let macro_name = macro_name_path_seg.ident.to_string();

    Some((macro_name, meta_arg))
}

fn struct_fields_from_method_args(
    args: &Punctuated<FnArg, Comma>,
) -> (Vec<TokenStream>, Vec<&Ident>, Vec<&Type>) {
    let mut struct_fields = vec![];
    let mut args_name_vec = vec![];
    let mut args_ty_vec = vec![];

    for arg in args {
        let FnArg::Typed(pat_ty) = arg else {
            continue;
        };
        let pat = &*pat_ty.pat;

        match pat {
            Pat::Ident(pat_ident) => {
                args_name_vec.push(&pat_ident.ident);
                args_ty_vec.push(pat_ty.ty.as_ref());
            }
            _ => unreachable!(),
        }
        struct_fields.push(quote! {
            #pat_ty,
        });
    }

    (struct_fields, args_name_vec, args_ty_vec)
}

// for example: for method `fn add(name: String, number: u64)` gives `contract.add(args.name, args.number)`
fn method_call_expr(method_name: &Ident, method_args: Vec<&Ident>) -> Expr {
    let mut args_sequence: Punctuated<Expr, Comma> = Punctuated::new();

    for arg in method_args {
        args_sequence.push(Expr::Field(ExprField {
            attrs: vec![],
            base: Box::new(Expr::Path(ExprPath {
                attrs: vec![],
                qself: None,
                path: Path::from(Ident::new("args", Span::call_site())),
            })),
            dot_token: Dot::default(),
            member: Member::Named(arg.clone()),
        }))
    }

    Expr::MethodCall(ExprMethodCall {
        attrs: vec![],
        receiver: Box::new(Expr::Path(ExprPath {
            attrs: vec![],
            qself: None,
            path: Path::from(Ident::new("smart_contract", Span::call_site())),
        })),
        dot_token: Dot::default(),
        method: method_name.clone(),
        turbofish: None,
        paren_token: Paren::default(),
        args: args_sequence,
    })
}

fn classmethod_call_expr(
    struct_name: &TypePath,
    method_name: &Ident,
    method_args: Vec<&Ident>,
) -> Expr {
    let mut args_sequence: Punctuated<Expr, Comma> = Punctuated::new();

    for arg in method_args {
        args_sequence.push(Expr::Field(ExprField {
            attrs: vec![],
            base: Box::new(Expr::Path(ExprPath {
                attrs: vec![],
                qself: None,
                path: Path::from(Ident::new("args", Span::call_site())),
            })),
            dot_token: Dot::default(),
            member: Member::Named(arg.clone()),
        }))
    }

    let mut name_path_segment: Punctuated<PathSegment, Token![::]> =
        struct_name.path.segments.clone();
    name_path_segment.push(PathSegment {
        ident: method_name.clone(),
        arguments: PathArguments::None,
    });

    Expr::Call(ExprCall {
        attrs: vec![],
        func: Box::new(Expr::Path(ExprPath {
            attrs: vec![],
            qself: None,
            path: Path {
                leading_colon: None,
                segments: name_path_segment,
            },
        })),
        paren_token: Paren::default(),
        args: args_sequence,
    })
}

fn try_method_call_or_return_control_flow(
    method_name: &Ident,
    method_call_expr: &Expr,
    return_ty: &ReturnType,
    is_async: bool,
) -> (TokenStream, TokenStream) {
    let non_result_return_ty_method_call_expr = if is_async {
        quote! {
            let result = weil_rs::runtime::Runtime::spawn_task(#method_call_expr);
        }
    } else {
        quote! {
            let result = #method_call_expr;
        }
    };

    let ReturnType::Type(_, return_ty) = return_ty else {
        return (non_result_return_ty_method_call_expr, quote! {()});
    };

    let resp = (non_result_return_ty_method_call_expr, quote! {#return_ty});

    let Type::Path(ty) = return_ty.as_ref() else {
        return resp;
    };

    let path = &ty.path.segments;
    let Some(core_ty) = path.last() else {
        return resp;
    };

    let method_name_str_literal = Literal::string(&method_name.to_string());

    if core_ty.ident.to_string() != "Result" {
        return resp;
    }

    let generic_args = &core_ty.arguments;

    let return_ty_token_stream: TokenStream = match generic_args {
        PathArguments::AngleBracketed(generic_args) => {
            let generic_args_segment = &generic_args.args;
            // if generic arguments inside `Result<...>` if more than two than probably it's some different type
            if generic_args_segment.len() > 2 {
                return resp;
            }

            let GenericArgument::Type(ty) = generic_args_segment.first().unwrap() else {
                return resp;
            };

            quote! {#ty}
        }
        PathArguments::None | PathArguments::Parenthesized(_) => return resp,
    };

    if is_async {
        return (
            quote! {
                let result = match weil_rs::runtime::Runtime::spawn_task(#method_call_expr) {
                    Ok(val) => val,
                    Err(err) => {
                        return Err(weil_rs::errors::WeilError::new_function_returned_with_error(#method_name_str_literal.to_string(), err))
                    }
                };
            },
            return_ty_token_stream,
        );
    } else {
        return (
            quote! {
                let result = match #method_call_expr {
                    Ok(val) => val,
                    Err(err) => {
                        return Err(weil_rs::errors::WeilError::new_function_returned_with_error(#method_name_str_literal.to_string(), err))
                    }
                };
            },
            return_ty_token_stream,
        );
    }
}

fn wasm_inteface_template(
    struct_path: &TypePath,
    method_name: &Ident,
    return_ty: &ReturnType,
    args_struct_name: &Ident,
    method_args: Vec<&Ident>,
    method_arg_types: Vec<&Type>,
    method_annotation_kind: SmartContractMethodAnnotationKind,
    macro_name: String,
    is_async: bool,
) -> Option<TokenStream> {
    let method_name_str = method_name.to_string();
    let method_name_str_literal = Literal::string(&method_name_str);
    let is_args = method_args.len() != 0;

    let args_snippet = if is_args {
        quote! {
            let args = match args {
                Ok(args) => args,
                Err(err) => return Err(weil_rs::errors::WeilError::new_method_argument_deserialization_error(#method_name_str_literal.to_string(), err))
            };
        }
    } else {
        quote! {}
    };

    let state_and_args_snippet = if is_args {
        if macro_name == "callback" {
            let method_arg_ty = method_arg_types[1];

            let Some((ok_ty, _)) = ok_err_ty_from_result(method_arg_ty) else {
                return None;
            };

            quote! {
                let (mut smart_contract, mut args) = weil_rs::runtime::Runtime::state_and_args::<#struct_path, CallbackArgs>();

                let args = match args {
                    Ok(args) => args,
                    Err(err) => return Err(weil_rs::errors::WeilError::new_method_argument_deserialization_error(#method_name_str_literal.to_string(), err))
                };

                let xpod_id = args.xpod_id;
                let args = weil_rs::utils::try_into_result::<#ok_ty>(args.result);

                struct Args {
                    result: #method_arg_ty,
                    xpod_id: String
                }

                let args = Args {
                    result: args,
                    xpod_id,
                };
            }
        } else {
            quote! {
                let (mut smart_contract, mut args) = weil_rs::runtime::Runtime::state_and_args::<#struct_path, #args_struct_name>();
                #args_snippet
            }
        }
    } else {
        quote! {
            let mut smart_contract = weil_rs::runtime::Runtime::state::<#struct_path>();
        }
    };

    match method_annotation_kind {
        SmartContractMethodAnnotationKind::Mutation => {
            let core_method_name =
                Ident::new(&format!("{}_core", &method_name_str), Span::call_site());
            let method_call_expr = method_call_expr(method_name, method_args);
            let (method_call_control_flow, return_ty) = try_method_call_or_return_control_flow(
                method_name,
                &method_call_expr,
                return_ty,
                is_async,
            );

            Some(quote! {
                fn #core_method_name() -> Result<weil_rs::runtime::WeilValue<#struct_path, #return_ty>, weil_rs::errors::WeilError> {
                    #state_and_args_snippet

                    #method_call_control_flow

                    Ok(weil_rs::runtime::WeilValue::new_with_state_and_ok_value(smart_contract, result))
                }

                #[no_mangle]
                pub extern "C" fn #method_name() {
                    let result = #core_method_name();
                    weil_rs::runtime::Runtime::set_state_and_result(result);
                }
            })
        }
        SmartContractMethodAnnotationKind::Query
        | SmartContractMethodAnnotationKind::QueryStream
        | SmartContractMethodAnnotationKind::QueryPlottable => {
            let core_method_name =
                Ident::new(&format!("{}_core", &method_name_str), Span::call_site());
            let method_call_expr = method_call_expr(method_name, method_args);
            let (method_call_control_flow, return_ty) = try_method_call_or_return_control_flow(
                method_name,
                &method_call_expr,
                return_ty,
                is_async,
            );

            Some(quote! {
                fn #core_method_name() -> Result<#return_ty, weil_rs::errors::WeilError> {
                    #state_and_args_snippet

                    #method_call_control_flow

                    Ok(result)
                }

                #[no_mangle]
                pub extern "C" fn #method_name() {
                    let result = #core_method_name();
                    weil_rs::runtime::Runtime::set_result(result);
                }
            })
        }
        SmartContractMethodAnnotationKind::Constructor(is_async) => {
            let init_call_expr = classmethod_call_expr(struct_path, method_name, method_args);
            let (method_call_control_flow, _) = try_method_call_or_return_control_flow(
                method_name,
                &init_call_expr,
                return_ty,
                is_async,
            );

            let args_snippet = if is_args {
                quote! {
                    let args = weil_rs::runtime::Runtime::args::<#args_struct_name>();
                    #args_snippet
                }
            } else {
                quote! {}
            };

            Some(quote! {
                fn core_init() -> Result<weil_rs::runtime::WeilValue<#struct_path, ()>, weil_rs::errors::WeilError> {
                    #args_snippet

                    #method_call_control_flow

                    Ok(weil_rs::runtime::WeilValue::new_with_state_and_ok_value(result, ()))
                }

                #[no_mangle]
                pub extern "C" fn init() {
                    let result = core_init();
                    weil_rs::runtime::Runtime::set_state_and_result(result);
                }
            })
        }
    }
}

pub fn impl_smart_contract_macro(impl_smart_contract: ItemImpl) -> proc_macro::TokenStream {
    let mut method_args_struct_vec = vec![];
    let mut wasm_interface_funcs = vec![];
    let mut method_kind_mapping = vec![];
    let self_ty = &*impl_smart_contract.self_ty;

    let Type::Path(struct_path) = self_ty else {
        return proc_macro::TokenStream::from(
            syn::Error::new_spanned(
                self_ty,
                "`smart_contract` macro can only be applied to `impl` associated with the WeilApplet state struct",
            )
            .to_compile_error(),
        )
        .into();
    };

    let mut has_constructor: Option<&ImplItemFn> = None;

    for item in &impl_smart_contract.items {
        let ImplItem::Fn(method) = item else {
            continue;
        };

        let method_name = &method.sig.ident;
        let attrs = &method.attrs;
        let is_async = method.sig.asyncness.is_some();

        if attrs.len() == 0 {
            continue;
        }

        let mut index = 0;

        while let Some(ident) = attrs[index].meta.path().get_ident() {
            if ident.to_string() == "doc" {
                index += 1;
            } else {
                break;
            }
        }

        let attr = &attrs[index];
        let meta = &attr.meta;

        let Some((macro_name, macro_arg)) = process_method_macro(meta) else {
            continue;
        };

        let method_annotation_kind =
            if macro_name == "mutate" || macro_name == "xpod" || macro_name == "callback" {
                SmartContractMethodAnnotationKind::Mutation
            } else if macro_name == "query" {
                if let Some(macro_arg) = macro_arg {
                    if macro_arg == "stream" {
                        SmartContractMethodAnnotationKind::QueryStream
                    } else if macro_arg == "plottable" {
                        SmartContractMethodAnnotationKind::QueryPlottable
                    } else {
                        SmartContractMethodAnnotationKind::Query
                    }
                } else {
                    SmartContractMethodAnnotationKind::Query
                }
            } else if macro_name == "constructor" {
                if let Some(previous_span) = has_constructor {
                    return proc_macro::TokenStream::from(
                        syn::Error::new_spanned(
                            &previous_span.sig.ident,
                            "duplicate constructor definition found, first one is declared here",
                        )
                        .to_compile_error(),
                    )
                    .into();
                } else {
                    has_constructor = Some(method);
                }

                SmartContractMethodAnnotationKind::Constructor(is_async)
            } else {
                continue;
            };

        let kind_literal = Literal::string(&method_annotation_kind.to_string());
        let method_name_literal = Literal::string(&method_name.to_string());

        method_kind_mapping.push(quote! {
            method_kind_mapping.insert(#method_name_literal, #kind_literal);
        });

        let args_struct_name = Ident::new(
            &format!("{}Args", snake_to_camel_case(&method_name.to_string())),
            Span::call_site(),
        );

        let (struct_fields, args_name_vec, args_ty_vec) =
            struct_fields_from_method_args(&method.sig.inputs);

        method_args_struct_vec.push(quote! {
            #[derive(Debug, Deserialize)]
            struct #args_struct_name {
                #(#struct_fields)*
            }
        });

        if let Some(tokens) = wasm_inteface_template(
            struct_path,
            method_name,
            &method.sig.output,
            &args_struct_name,
            args_name_vec,
            args_ty_vec,
            method_annotation_kind,
            macro_name,
            is_async,
        ) {
            wasm_interface_funcs.push(tokens);
        }
    }

    let new_func = quote! {
        #[derive(Debug, Deserialize)]
        struct CallbackArgs {
            result: Result<String, weil_rs::errors::WeilError>,
            xpod_id: String,
        }

        #[no_mangle]
        pub extern "C" fn __new(len: usize, _id: u8) -> i32 {
            let ptr = weil_rs::runtime::Runtime::allocate(len);

            ptr as _
        }

        #[no_mangle]
        pub extern "C" fn __free(ptr: usize, len: usize) {
            weil_rs::runtime::Runtime::deallocate(ptr, len);
        }
    };

    let method_kind_data_func = quote! {
        #[no_mangle]
        pub extern "C" fn method_kind_data() {
            let mut method_kind_mapping: std::collections::BTreeMap<&'static str, &'static str> = std::collections::BTreeMap::default();

            #(#method_kind_mapping)*

            weil_rs::runtime::Runtime::set_result(Ok(method_kind_mapping));
        }
    };

    let init_func = if has_constructor.is_none() {
        quote! {
            #[no_mangle]
            pub extern "C" fn init() {
                let smart_contract_state = #struct_path::default();
                let result = Ok(weil_rs::runtime::WeilValue::new_with_state_and_ok_value(smart_contract_state, ()));
                weil_rs::runtime::Runtime::set_state_and_result(result);
            }
        }
    } else {
        quote! {}
    };

    quote! {
        #impl_smart_contract
        #(#method_args_struct_vec)*
        #new_func
        #method_kind_data_func
        #init_func
        #(#wasm_interface_funcs)*
    }
    .into()
}

fn check_macro_applied_method_signature(
    sig: &Signature,
    kind: &'static str,
) -> Result<(), proc_macro::TokenStream> {
    // TODO - implement all the constraints which signature should follow for example method should not be `async`,
    // or generic parameters.
    let Some(first_arg) = sig.inputs.first() else {
        return Err(proc_macro::TokenStream::from(
            syn::Error::new_spanned(
                sig,
                &format!("`{}` macro can only be applied to a method", kind),
            )
            .to_compile_error(),
        )
        .into());
    };

    let FnArg::Receiver(_) = first_arg else {
        return Err(proc_macro::TokenStream::from(
            syn::Error::new_spanned(
                sig,
                &format!("`{}` macro can only be applied to a method", kind),
            )
            .to_compile_error(),
        )
        .into());
    };

    Ok(())
}

fn impl_smart_contract_method_macro(item: ImplItemFn) -> proc_macro::TokenStream {
    let sig = &item.sig;

    if let Err(err) = check_macro_applied_method_signature(sig, "mutate") {
        return err;
    };

    quote! {
        #item
    }
    .into()
}

pub fn impl_smart_contract_mutate_macro(
    smart_contract_mutate_method: ImplItemFn,
) -> proc_macro::TokenStream {
    impl_smart_contract_method_macro(smart_contract_mutate_method)
}

pub fn impl_smart_contract_query_macro(
    smart_contract_query_method: ImplItemFn,
    query_opaque_kind: QueryOpaqueKind,
) -> proc_macro::TokenStream {
    if !query_opaque_kind.is_ordinary()
        && !validate_query_opaque_return_type(&smart_contract_query_method.sig.output)
    {
        return proc_macro::TokenStream::from(
            syn::Error::new_spanned(
                smart_contract_query_method.sig.output.clone(),
                "#query(stream) or #query(plottable) macro can only have return types that contain ByteStream or Plottable",
            )
            .to_compile_error(),
        )
        .into();
    }

    impl_smart_contract_method_macro(smart_contract_query_method)
}

pub fn impl_smart_contract_secured_macro(
    arg_str: String,
    smart_contract_method: ImplItemFn,
) -> proc_macro::TokenStream {
    let mut curr_block = smart_contract_method.block.clone();

    let stream = quote! {
        {
            let identity_addr = weil_rs::runtime::Runtime::contract_id_for_name(#arg_str);

            #[derive(Serialize)]
            struct IdentityArgs {
                key: String,
            }

            let identity_args = serde_json::to_string(&IdentityArgs {
                key: "key_management_addr".to_string(),
            })
            .unwrap();

            let key_manager_addr = weil_rs::runtime::Runtime::call_contract::<Option<String>>(
                identity_addr,
                "get_data".to_string(),
                Some(identity_args),
            )
            .map_err(|err| err.to_string())?;

            let Some(key_manager_addr) = key_manager_addr else {
                return Err("key manager address not found".to_string());
            };

            let addr = weil_rs::runtime::Runtime::sender();

            #[derive(Serialize)]
            struct Args {
                key: String,
                purpose: weil_contracts::key_management::KeyPurpose,
            }

            let args = Args {
                key: addr,
                purpose: weil_contracts::key_management::KeyPurpose::Execution,
            };

            let has_purpose = weil_rs::runtime::Runtime::call_contract::<bool>(
                key_manager_addr,
                "key_has_purpose".to_string(),
                Some(serde_json::to_string(&args).unwrap()),
            )
            .map_err(|err| err.to_string())?;

            if !has_purpose {
                return Err("sender address not authorized".to_string());
            }
        }
    };

    let mut new_block = parse::<Block>(stream.into()).unwrap();
    new_block.stmts.append(&mut curr_block.stmts);

    let mut new_func = smart_contract_method.clone();
    new_func.block = new_block;

    quote! {
        #new_func
    }
    .into()
}

fn type_is_bytestream(ty: &Type) -> bool {
    if let Type::Path(ty_path) = ty {
        let last_token = ty_path.path.segments.last().unwrap();
        return last_token.ident.eq("ByteStream");
    } else {
        return false;
    }
}

fn type_is_plottable(ty: &Type) -> bool {
    if let Type::Path(ty_path) = ty {
        let last_token = ty_path.path.segments.last().unwrap();
        return last_token.ident.eq("Plottable");
    } else {
        return false;
    }
}

fn validate_query_opaque_return_type(return_type: &ReturnType) -> bool {
    let ReturnType::Type(_, ty) = return_type else {
        return false;
    };

    let Type::Path(ty_path) = &ty.as_ref() else {
        return false;
    };

    let segments = &ty_path.path.segments;
    let first_token = segments.first().unwrap();
    let last_token = &segments.last().unwrap().ident;

    // when solely ByteStream is returned
    if !first_token.ident.eq("Result") {
        return last_token.eq("ByteStream") || last_token.eq("Plottable");
    }

    if segments.len() != 1 {
        return false;
    };

    let PathArguments::AngleBracketed(generic_args) = &first_token.arguments else {
        return false;
    };

    let args = &generic_args.args;
    let first_arg = args.first().unwrap(); // After angle-bracker(<),
                                           // first argument will be the path of ByteStream
    if let GenericArgument::Type(ty) = first_arg {
        return type_is_bytestream(ty) || type_is_plottable(ty);
    } else {
        return false;
    }
}

pub fn impl_smart_contract_xpod_macro(
    smart_contract_query_method: ImplItemFn,
) -> proc_macro::TokenStream {
    let sig = &smart_contract_query_method.sig;

    if let Err(err) = check_macro_applied_method_signature(sig, "mutate") {
        return err;
    };

    quote! {
        #smart_contract_query_method
    }
    .into()
}

pub fn impl_smart_contract_callback_macro(
    smart_contract_query_method: ImplItemFn,
    assosiated_method_name: String,
) -> proc_macro::TokenStream {
    let sig = &smart_contract_query_method.sig;

    if let Err(err) = check_macro_applied_method_signature(sig, "mutate") {
        return err;
    };

    let name = sig.ident.to_string();

    if name != format!("{}_callback", assosiated_method_name) {
        return proc_macro::TokenStream::from(
            syn::Error::new_spanned(
                &sig.output,
                format!("callback function name should be of the form `{{assosiated_func_name}}_callback`, instead got `{}`", name),
            )
            .to_compile_error(),
        )
        .into();
    }

    let err_tokens: proc_macro::TokenStream = proc_macro::TokenStream::from(
        syn::Error::new_spanned(
            &sig.inputs,
            "callback function should only have two parameter, `xpod_id` with type `String` and `result` with type of the form `Result<T, String>`",
        )
        .to_compile_error(),
    )
    .into();

    if sig.inputs.len() != 3 {
        return err_tokens;
    }

    let mut params_iter = sig.inputs.iter();
    params_iter.next().unwrap(); // ignore the first param with is `&mut self`

    let xpod_id_param = params_iter.next().unwrap();
    let result_param = params_iter.next().unwrap();

    let FnArg::Typed(xpod_id_ty) = xpod_id_param else {
        return err_tokens;
    };

    let FnArg::Typed(result_ty) = result_param else {
        return err_tokens;
    };

    let xpod_id_ty = &xpod_id_ty.ty;
    let result_ty = &result_ty.ty;

    if !is_eq(xpod_id_ty, "String") {
        return err_tokens;
    }

    if let Some((_, err_ty)) = ok_err_ty_from_result(result_ty) {
        if !is_eq(err_ty, "String") {
            return err_tokens;
        }
    } else {
        return err_tokens;
    }

    quote! {
        #smart_contract_query_method
    }
    .into()
}

pub fn impl_smart_contract_constructor_macro(
    smart_contract_constructor: ImplItemFn,
) -> proc_macro::TokenStream {
    let sig = &smart_contract_constructor.sig;

    if let Some(first_arg) = sig.inputs.first() {
        if let FnArg::Receiver(receiver) = first_arg {
            return proc_macro::TokenStream::from(
                syn::Error::new_spanned(
                    receiver,
                    "`constructor` macro can only be applied to a classmethod with no receiver",
                )
                .to_compile_error(),
            )
            .into();
        }
    }

    // TODO - check if ty is `Self` or Result<Self, ...>
    let ReturnType::Type(_, ty) = &sig.output else {
        return proc_macro::TokenStream::from(
            syn::Error::new_spanned(
                &sig.output,
                "`constructor` macro can only be applied to a classmethod which returns `Self`",
            )
            .to_compile_error(),
        )
        .into();
    };

    quote! {
        #smart_contract_constructor
    }
    .into()
}
