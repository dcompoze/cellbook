use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::visit_mut::VisitMut;
use syn::{parse_macro_input, FnArg, ItemFn};

/// Adds `ctx` prefix to context macro calls.
struct CtxInjector;

impl VisitMut for CtxInjector {
    fn visit_macro_mut(&mut self, mac: &mut syn::Macro) {
        let path = &mac.path;
        let is_context_macro = path.is_ident("store")
            || path.is_ident("load")
            || path.is_ident("remove")
            || path.is_ident("consume");

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
