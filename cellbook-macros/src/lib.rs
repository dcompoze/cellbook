use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, ItemFn};

/// Marks an async function as a cellbook cell and registers it.
#[proc_macro_attribute]
pub fn cell(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    let fn_name = &input.sig.ident;
    let fn_name_str = fn_name.to_string();
    let wrapper_name = format_ident!("__cellbook_{}", fn_name);
    let line = fn_name.span().start().line as u32;

    let expanded = quote! {
        #input

        #[doc(hidden)]
        fn #wrapper_name() -> ::cellbook::futures::future::BoxFuture<'static, ::cellbook::Result<()>> {
            Box::pin(#fn_name())
        }

        ::cellbook::inventory::submit!(::cellbook::CellInfo {
            name: #fn_name_str,
            func: #wrapper_name,
            line: #line,
        });
    };

    TokenStream::from(expanded)
}
