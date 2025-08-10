use proc_macro::TokenStream;
use proc_macro2::{Ident, Span, TokenStream as TokenStream2, TokenTree};
use quote::quote;
use std::str::FromStr;

enum MatchElement {
    Number,
    Plus,
    Multiply,
    Pow,
    Variable,
    Function,
}

impl MatchElement {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "num" => Some(MatchElement::Number),
            "plus" => Some(MatchElement::Plus),
            "mul" => Some(MatchElement::Multiply),
            "pow" => Some(MatchElement::Pow),
            "var" => Some(MatchElement::Variable),
            "fun" => Some(MatchElement::Function),
            _ => None,
        }
    }

    fn as_str(&self) -> &str {
        match self {
            MatchElement::Number => "num",
            MatchElement::Plus => "plus",
            MatchElement::Multiply => "mul",
            MatchElement::Pow => "pow",
            MatchElement::Variable => "var",
            MatchElement::Function => "fun",
        }
    }

    fn as_pattern(&self) -> TokenTree {
        let name = match self {
            MatchElement::Number => "Number",
            MatchElement::Plus => "Plus",
            MatchElement::Multiply => "Multiply",
            MatchElement::Pow => "Pow",
            MatchElement::Variable => "Variable",
            MatchElement::Function => "Function",
        };
        TokenTree::Ident(Ident::new(name, Span::call_site()))
    }
}

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
    let match_element = MatchElement::from_str(&name).expect("unexpected element to match on");
    if let Some(operation_args) = inner_elements {
        match match_element {
            MatchElement::Number => {
                if operation_args.len() != 1 || operation_args[0].len() != 1 {
                    panic!("expected exactly one inner element for num");
                }
                match &operation_args[0][0] {
                    TokenTree::Ident(i) => {
                        if i.to_string() != "x" {
                            panic!("expected identifier 'x' as inner element for num, got '{}'", i);
                        }
                        quote! {
                            {
                                let __expr = #expr_ts;
                                match __expr {
                                    Element::Number(n) => Some(n),
                                    _ => None,
                                }
                            }
                        }
                    },
                    TokenTree::Literal(l) => {
                        quote! {
                            {
                                let __expr = #expr_ts;
                                match __expr {
                                    Element::Number(n) => {
                                        (n != #l).then_some(())
                                    },
                                    _ => None
                                }
                            }
                        }
                    },
                    _ => panic!("expected identifier or literal as inner element for num"),
                }
            },
            MatchElement::Plus => todo!(),
            MatchElement::Multiply => todo!(),
            MatchElement::Pow => {
                if operation_args.len() != 2 {
                    panic!("expected exactly two inner elements for pow");
                }
                let base: TokenStream2 = operation_args[0].clone().into_iter().collect();
                let exponent: TokenStream2 = operation_args[1].clone().into_iter().collect();

                let include_base = base.to_string().contains("x");
                let include_exponent = exponent.to_string().contains("x");

                let base_input = if base.to_string() == "x" {
                    quote! { b }
                } else {
                    quote! { match_formula_proc!(b.as_ref(), #base)? }
                };
                let exponent_input = if exponent.to_string() == "x" {
                    quote! { e }
                } else {
                    quote! { match_formula_proc!(e.as_ref(), #exponent)? }
                };

                let outputs = [(include_base, "base"), (include_exponent, "exponent")];

                let string = outputs.iter().filter(|x| x.0).map(|x| x.1).collect::<Vec<_>>().join(", ");
                let output = TokenStream2::from_str(&format!("Some(({}))", string)).unwrap();
                quote! {
                    (||{
                        let __expr = &#expr_ts;
                        match &__expr {
                            Element::Pow(b, e) => {
                                let base = #base_input;
                                let exponent = #exponent_input;
                                #output
                            },
                            _ => None
                        }
                    })()
                }
            },
            MatchElement::Variable => todo!(),
            MatchElement::Function => todo!(),
        }
    } else {
        let match_name = match_element.as_pattern();
        quote! {
            'outer: {
                let __expr = #expr_ts;
                if !matches!(__expr, Element::#match_name {..}) { break 'outer None; }
                Some(())
            }
        }
    }
        .into()
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
