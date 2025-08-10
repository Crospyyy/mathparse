use proc_macro::TokenStream;
use proc_macro2::{TokenStream as TokenStream2, TokenTree};
use quote::quote;
use std::str::FromStr;

#[proc_macro]
pub fn match_formula_proc(item: TokenStream) -> TokenStream {
    let input: TokenStream2 = item.into();
    let mut iter = input.into_iter();
    // expr bis Komma
    let mut expr_tokens = Vec::new();
    while let Some(tt) = iter.next() {
        if let TokenTree::Punct(p) = &tt {
            if p.as_char() == ',' {
                break;
            }
        }
        expr_tokens.push(tt);
    }
    let expr_ts: TokenStream2 = expr_tokens.into_iter().collect();
    // ident
    let ident = match iter.next() {
        Some(TokenTree::Ident(i)) => i,
        _ => panic!("expected identifier"),
    };
    // optionale innere Tokentrees
    let inner_group = iter.next().map(|tt| match tt {
        TokenTree::Group(g) => g.stream(),
        _ => panic!("unexpected token after identifier"),
    });
    if iter.next().is_some() {
        panic!("unexpected tokens after inner group");
    }
    let name = ident.to_string();
    let inner_group = inner_group.unwrap_or(TokenStream2::from_str("()").unwrap());
    let expanded = quote! {
        {
            let __expr = #expr_ts;
            if !matches!(__expr, Element::Number(_)) {
                None
            } else {
                Some(())
            }
        }
    };
    expanded.into()
}
