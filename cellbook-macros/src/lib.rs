use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::visit_mut::VisitMut;
use syn::{parse_macro_input, FnArg, ItemFn};

/// Visitor that adds `ctx` prefix to store!, load!, remove!, consume! macro calls
struct CtxInjector;

impl VisitMut for CtxInjector {
    fn visit_macro_mut(&mut self, mac: &mut syn::Macro) {
        let path = &mac.path;
        let is_context_macro = path.is_ident("store")
            || path.is_ident("load")
            || path.is_ident("remove")
            || path.is_ident("consume");

        if is_context_macro {
            // Prepend `ctx, ` to the macro tokens
            let tokens = &mac.tokens;
            mac.tokens = quote! { ctx, #tokens };
        }
    }
}

/// Marks an async function as a cellbook cell and registers it automatically.
///
/// The macro transforms the function to:
/// 1. Accept a `ctx: CellContext` parameter
/// 2. Generate a `#[no_mangle]` wrapper for FFI
/// 3. Register the cell with inventory for automatic discovery
///
/// # Example
///
/// ```ignore
/// #[cell]
/// async fn my_cell() -> Result<()> {
///     let data = vec![1, 2, 3];
///     store!(data)?;  // ctx is automatically available
///     Ok(())
/// }
/// ```
#[proc_macro_attribute]
pub fn cell(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as ItemFn);

    // Get name and line before mutable borrow
    let fn_name = input.sig.ident.clone();
    let fn_name_str = fn_name.to_string();
    let wrapper_name = format_ident!("__cellbook_cell_{}", fn_name_str);
    let line = fn_name.span().start().line as u32;

    // Transform store!/load!/remove!/consume! calls to include ctx
    CtxInjector.visit_item_fn_mut(&mut input);

    // Add ctx parameter to the function signature
    let ctx_param: FnArg = syn::parse_quote!(ctx: &::cellbook::CellContext);
    input.sig.inputs.insert(0, ctx_param);

    // Extract the function body and other parts
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
                #fn_name(&ctx).await.map_err(|e| Box::new(e) as Box<dyn ::std::error::Error + Send + Sync>)
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

/// Generates the cell discovery and config functions for the dylib.
///
/// Call this macro once at the end of your cellbook.rs to export all registered cells.
/// Optionally pass a `Config` to customize behavior.
///
/// # Examples
///
/// ```ignore
/// #[cell]
/// async fn hello() -> Result<()> { Ok(()) }
///
/// #[cell]
/// async fn world() -> Result<()> { Ok(()) }
///
/// // Using defaults
/// cellbook!();
///
/// // Using struct literal
/// cellbook!(Config {
///     auto_reload: false,
///     ..Default::default()
/// });
///
/// // Using builder methods
/// cellbook!(Config::default()
///     .auto_reload(false)
///     .image_viewer("feh"));
/// ```
#[proc_macro]
pub fn cellbook(input: TokenStream) -> TokenStream {
    // Parse optional config expression
    let config_expr = if input.is_empty() {
        quote! { ::cellbook::Config::default() }
    } else {
        let expr = parse_macro_input!(input as syn::Expr);
        quote! { #expr }
    };

    let expanded = quote! {
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
        pub extern "Rust" fn __cellbook_get_config() -> ::cellbook::Config {
            #config_expr
        }
    };

    TokenStream::from(expanded)
}
