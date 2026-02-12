use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::visit_mut::VisitMut;
use syn::{DeriveInput, Expr, ExprLit, FnArg, ItemFn, Lit, Meta, MetaNameValue, parse_macro_input};

/// Adds `ctx` prefix to context macro calls.
struct CtxInjector;

impl VisitMut for CtxInjector {
    fn visit_macro_mut(&mut self, mac: &mut syn::Macro) {
        let path = &mac.path;
        let is_context_macro = path.is_ident("store")
            || path.is_ident("storev")
            || path.is_ident("load")
            || path.is_ident("loadv")
            || path.is_ident("remove")
            || path.is_ident("consume")
            || path.is_ident("consumev");

        if is_context_macro {
            let tokens = &mac.tokens;
            mac.tokens = quote! { ctx, #tokens };
        }
    }
}

/// Marks an async function as a cellbook cell.
///
/// The macro:
/// - Adds a `ctx: CellContext` parameter
/// - Generates a `#[no_mangle]` wrapper for FFI
/// - Registers the cell with inventory
///
/// ```ignore
/// #[cell]
/// async fn my_cell() -> Result<()> {
///     store!(data)?;
///     Ok(())
/// }
/// ```
#[proc_macro_attribute]
pub fn cell(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as ItemFn);

    let fn_name = input.sig.ident.clone();
    let fn_name_str = fn_name.to_string();
    let wrapper_name = format_ident!("__cellbook_cell_{}", fn_name_str);
    let line = fn_name.span().start().line as u32;

    CtxInjector.visit_item_fn_mut(&mut input);

    let ctx_param: FnArg = syn::parse_quote!(ctx: &::cellbook::CellContext);
    input.sig.inputs.insert(0, ctx_param);

    let fn_vis = &input.vis;
    let fn_sig = &input.sig;
    let fn_block = &input.block;
    let fn_attrs = &input.attrs;

    let expanded = quote! {
        #(#fn_attrs)*
        #fn_vis #fn_sig #fn_block

        #[doc(hidden)]
        #[unsafe(no_mangle)]
        pub fn #wrapper_name(
            store_fn: fn(&str, Vec<u8>, &str),
            load_fn: fn(&str) -> Option<(Vec<u8>, String)>,
            remove_fn: fn(&str) -> Option<(Vec<u8>, String)>,
            list_fn: fn() -> Vec<(String, String)>,
        ) -> ::cellbook::futures::future::BoxFuture<'static, ::std::result::Result<(), Box<dyn ::std::error::Error + Send + Sync>>> {
            let ctx = ::cellbook::CellContext::new(store_fn, load_fn, remove_fn, list_fn);
            Box::pin(async move {
                #fn_name(&ctx)
                    .await
                    .map_err(|e| -> Box<dyn ::std::error::Error + Send + Sync> { e.into() })
            })
        }

        ::cellbook::inventory::submit!(::cellbook::CellInfo {
            name: #fn_name_str,
            func: #wrapper_name,
            line: #line,
        });
    };

    TokenStream::from(expanded)
}

/// Marks an async function as the required cellbook init entrypoint.
///
/// The macro:
/// - Keeps the function as-is (arbitrary function name)
/// - Exports `__cellbook_get_cells`
/// - Exports `__cellbook_get_init`
///
/// ```ignore
/// #[init]
/// async fn setup() -> Result<()> {
///     Ok(())
/// }
/// ```
#[proc_macro_attribute]
pub fn init(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let fn_name = input.sig.ident.clone();
    let fn_name_str = fn_name.to_string();
    let wrapper_name = format_ident!("__cellbook_init_{}", fn_name_str);
    let line = fn_name.span().start().line as u32;

    let fn_vis = &input.vis;
    let fn_sig = &input.sig;
    let fn_block = &input.block;
    let fn_attrs = &input.attrs;

    let expanded = quote! {
        #(#fn_attrs)*
        #fn_vis #fn_sig #fn_block

        #[doc(hidden)]
        #[unsafe(no_mangle)]
        pub fn #wrapper_name() -> ::cellbook::futures::future::BoxFuture<'static, ::std::result::Result<(), Box<dyn ::std::error::Error + Send + Sync>>> {
            Box::pin(async move {
                #fn_name()
                    .await
                    .map_err(|e| -> Box<dyn ::std::error::Error + Send + Sync> { e.into() })
            })
        }

        #[unsafe(no_mangle)]
        pub extern "Rust" fn __cellbook_get_cells() -> Vec<(
            String,
            u32,
            fn(
                fn(&str, Vec<u8>, &str),
                fn(&str) -> Option<(Vec<u8>, String)>,
                fn(&str) -> Option<(Vec<u8>, String)>,
                fn() -> Vec<(String, String)>,
            ) -> ::cellbook::futures::future::BoxFuture<'static, ::std::result::Result<(), Box<dyn ::std::error::Error + Send + Sync>>>
        )> {
            ::cellbook::registry::cells()
                .into_iter()
                .map(|c| (c.name.to_string(), c.line, c.func))
                .collect()
        }

        #[unsafe(no_mangle)]
        pub extern "Rust" fn __cellbook_get_init() -> (
            String,
            u32,
            fn() -> ::cellbook::futures::future::BoxFuture<'static, ::std::result::Result<(), Box<dyn ::std::error::Error + Send + Sync>>>
        ) {
            (#fn_name_str.to_string(), #line, #wrapper_name)
        }
    };

    TokenStream::from(expanded)
}

/// Derive `cellbook::StoreSchema` with a version set by `#[store_schema(version = N)]`.
#[proc_macro_derive(StoreSchema, attributes(store_schema))]
pub fn derive_store_schema(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let ident = input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let mut version: Option<u32> = None;

    for attr in &input.attrs {
        if !attr.path().is_ident("store_schema") {
            continue;
        }

        let parsed = match attr
            .parse_args_with(syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated)
        {
            Ok(v) => v,
            Err(e) => return e.to_compile_error().into(),
        };

        for meta in parsed {
            let Meta::NameValue(MetaNameValue { path, value, .. }) = meta else {
                return syn::Error::new_spanned(attr, "expected #[store_schema(version = <u32>)]")
                    .to_compile_error()
                    .into();
            };

            if !path.is_ident("version") {
                return syn::Error::new_spanned(path, "unknown store_schema key")
                    .to_compile_error()
                    .into();
            }

            let Expr::Lit(ExprLit {
                lit: Lit::Int(lit_int),
                ..
            }) = value
            else {
                return syn::Error::new_spanned(value, "version must be an integer literal")
                    .to_compile_error()
                    .into();
            };

            match lit_int.base10_parse::<u32>() {
                Ok(v) => version = Some(v),
                Err(e) => {
                    return syn::Error::new_spanned(lit_int, e).to_compile_error().into();
                }
            }
        }
    }

    let Some(version) = version else {
        return syn::Error::new_spanned(
            &ident,
            "missing #[store_schema(version = <u32>)] for #[derive(StoreSchema)]",
        )
        .to_compile_error()
        .into();
    };

    let expanded = quote! {
        impl #impl_generics ::cellbook::StoreSchema for #ident #ty_generics #where_clause {
            const VERSION: u32 = #version;
        }
    };
    TokenStream::from(expanded)
}
