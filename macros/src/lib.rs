use proc_macro::TokenStream;
use proc_macro2::{Ident, Span, TokenStream as TokenStream2, TokenTree};
use quote::__private::ext::RepToTokensExt;
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

    let input_args = split_by_comma(input);
    if input_args.len() != 2 {
        panic!("expected exactly two arguments");
    }
    let mut input_args_iter = input_args.into_iter();

    let formula = input_args_iter.next().unwrap();
    let match_expr = input_args_iter.next().unwrap();

    let formula_ts: TokenStream2 = formula.into_iter().collect();

    // ident
    if match_expr.len() > 2 {
        panic!("Expected no more than two tokens in match expression");
    }
    let match_ident = match match_expr.get(0) {
        Some(TokenTree::Ident(i)) => i,
        _ => panic!("expected identifier"),
    };
    let inner_elements = match_expr.get(1).map(|tt| match tt {
        TokenTree::Group(g) => split_by_comma(g.stream()),
        _ => panic!("unexpected token after identifier"),
    });

    let name = match_ident.to_string();
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
                                let __expr = #formula_ts;
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
                                let __expr = #formula_ts;
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

                // todo create some code that works for an arbitrary number of inputs to process and outputs
                // Naming of the inner variables could be like 'var_0', 'var_1', ..., 'var_n'
                // Only count the variables, which actually have a value

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
                        let __expr = &#formula_ts;
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
                let __expr = #formula_ts;
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
