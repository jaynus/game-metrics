#![deny(clippy::pedantic, clippy::all)]
#![allow(
    clippy::must_use_candidate,
    clippy::new_ret_no_self,
    clippy::cast_precision_loss,
    clippy::missing_safety_doc,
    clippy::default_trait_access,
    clippy::module_name_repetitions,
    non_upper_case_globals
)]
extern crate proc_macro;

use crate::proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, parse_quote, AttributeArgs, Item, LitStr, Meta, NestedMeta};

#[cfg(not(feature = "disable"))]
#[proc_macro_attribute]
pub fn instrument(attr: TokenStream, input: TokenStream) -> TokenStream {
    let attrs = parse_macro_input!(attr as AttributeArgs);

    let name = attrs
        .iter()
        .find_map(|attr| {
            if let NestedMeta::Meta(meta) = attr {
                if let Meta::NameValue(kv) = meta {
                    if kv.path.is_ident("name") {
                        if let syn::Lit::Str(s) = &kv.lit {
                            return Some(s.clone());
                        }
                    }
                }
            }
            None
        });

    let inner = parse_macro_input!(input as Item);
    match inner {
        Item::Fn(mut f) => {
            let name = name.unwrap_or(LitStr::new(&f.sig.ident.to_string(), proc_macro2::Span::call_site()));

            let block = f.block;
            f.block = Box::new(parse_quote! {
                {
                    {
                        let __span = game_metrics::Span::new(#name);
                        #block
                    }
                }
            });
            TokenStream::from(quote! { #f })
        }
        _ => panic!("unsupported type for the #[instrument] macro"),
    }
}

#[cfg(feature = "disable")]
#[proc_macro_attribute]
pub fn instrument(attr: TokenStream, input: TokenStream) -> TokenStream {
    input.into()
}

