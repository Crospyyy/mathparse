use proc_macro::TokenStream;
use proc_macro2::{TokenStream as TokenStream2, TokenTree};
use quote::__private::ext::RepToTokensExt;
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
    let inner_elements = iter.next().map(|tt| match tt {
        TokenTree::Group(g) => split_by_comma(g.stream()),
        _ => panic!("unexpected token after identifier"),
    });
    if iter.next().is_some() {
        panic!("unexpected tokens after inner group");
    }
    let name = ident.to_string();
    let expanded = match name.as_str() {
        "num" => {
            if let Some(inner) = inner_elements {
                if inner.len() != 1 || inner[0].len() != 1 {
                    panic!("expected exactly one inner element for num");
                }
                match &inner[0][0] {
                    TokenTree::Ident(i) => {
                        if i.to_string() != "x" {
                            panic!("expected identifier 'x' as inner element for num, got '{}'", i);
                        }
                        quote! {
                            'outer: {
                                let __expr = #expr_ts;
                                match __expr {
                                    Element::Number(n) => Some(n),
                                    _ => break 'outer None
                                }
                            }
                        }
                    },
                    TokenTree::Literal(l) => {
                        quote! {
                            'outer: {
                                let __expr = #expr_ts;
                                match __expr {
                                    Element::Number(n) => {
                                        if n != #l { break 'outer None; }
                                    },
                                    _ => break 'outer None
                                }
                                Some(())
                            }
                        }
                    },
                    _ => panic!("expected identifier or literal as inner element for num"),
                }
            } else {
                quote! {
                    'outer: {
                        let __expr = #expr_ts;
                        if !matches!(__expr, Element::Number(_)) { break 'outer None; }
                        Some(())
                    }
                }
            }
        },
        _ => panic!("unexpected element to match on."),
    };
    expanded.into()
}

fn split_by_comma(ts: TokenStream2) -> Vec<Vec<TokenTree>> {
    // initialisiere ergebnisvektor
    let mut result = Vec::new();
    // sammle tokens bis zum kommatrennzeichen
    let mut segment = Vec::new();
    for tt in ts.into_iter() {
        if let TokenTree::Punct(p) = &tt {
            if p.as_char() == ',' {
                result.push(segment);
                segment = Vec::new();
                continue;
            }
        }
        segment.push(tt);
    }
    // letztes segment hinzufügen wenn nicht leer
    if !segment.is_empty() {
        result.push(segment);
    }
    result
}
