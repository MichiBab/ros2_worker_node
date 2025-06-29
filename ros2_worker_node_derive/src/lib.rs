use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

#[proc_macro_derive(WorkerNode)]
pub fn derive_work_scope(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;

    let expanded = quote! {
        impl rclrs::WorkScope for #name {
            type Payload = Self;
        }
    };

    TokenStream::from(expanded)
}
