use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use quote::quote;

/// Generates a struct name from the current crate and implements RegisteredBin
#[proc_macro]
pub fn kestrelize(_input: TokenStream) -> TokenStream {
	let crate_name = std::env!("CARGO_PKG_NAME");
	let struct_name = crate_name.to_case(Case::Pascal); // e.g., "my-crate" -> "MyCrate"
	let ident = syn::Ident::new(&struct_name, proc_macro2::Span::call_site());

	TokenStream::from(quote! {
		pub struct #ident;

		impl kestrel::RegisteredBin for #ident {
			fn cargo_bin() -> &'static str {
				env!("CARGO_PKG_NAME")
			}
		}
	})
}
