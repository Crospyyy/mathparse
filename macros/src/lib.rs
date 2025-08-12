use proc_macro::TokenStream;
use proc_macro2::{Ident, Span, TokenStream as TokenStream2, TokenTree};
use quote::__private::ext::RepToTokensExt;
use quote::quote;
use std::str::FromStr;

enum MatchElement {
    Number,
    Negate,
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
            "neg" => Some(MatchElement::Negate),
            _ => None,
        }
    }

    fn as_str(&self) -> &str {
        match self {
            MatchElement::Number => "num",
            MatchElement::Negate => "neg",
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
            MatchElement::Negate => "Negate",
            MatchElement::Plus => "Plus",
            MatchElement::Multiply => "Multiply",
            MatchElement::Pow => "Pow",
            MatchElement::Variable => "Variable",
            MatchElement::Function => "Function",
        };
        TokenTree::Ident(Ident::new(name, Span::call_site()))
    }
}

struct MatchInput {
    formula: TokenStream2,
    match_ident: Ident,
    inner_elements: Option<Vec<Vec<TokenTree>>>,
}

#[proc_macro]
pub fn match_formula(item: TokenStream) -> TokenStream {
    let input: TokenStream2 = item.into();
    let MatchInput { formula, match_ident, inner_elements } = process_input(input);
    let stream = create_code(formula, match_ident, inner_elements);
    stream.into()
}

fn create_code(
    formula: TokenStream2, match_ident: Ident, inner_elements: Option<Vec<Vec<TokenTree>>>,
) -> TokenStream2 {
    if match_ident.to_string() == "_" && inner_elements.is_none() {
        return quote! {Some(())}.into();
    }

    let name = match_ident.to_string();
    let match_element = MatchElement::from_str(&name).expect("unexpected element to match on");
    let match_stream: TokenStream2 = if let Some(operation_args) = inner_elements {
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
                        quote! { Element::Number(n) => Some(n) }
                    },
                    TokenTree::Literal(l) => {
                        quote! { Element::Number(n) => { (n == #l).then_some(()) } }
                    },
                    _ => panic!("expected identifier or literal as inner element for num"),
                }
            },
            MatchElement::Plus => {
                let expected_elements = operation_args.len();
                let args = operation_args.iter().cloned().map(|a| a.into_iter().collect::<TokenStream2>());
                let variables_ts_stream = create_variables(args.clone());
                let outputs_ts_stream = create_outputs(args);
                quote! {
                    Element::Plus(inputs) => {
                        if inputs.len() != #expected_elements {
                            return None;
                        }
                        #variables_ts_stream
                        #outputs_ts_stream
                    }
                }
            },
            MatchElement::Multiply => {
                let expected_elements = operation_args.len();
                let args = operation_args.iter().cloned().map(|a| a.into_iter().collect::<TokenStream2>());
                let variables_ts_stream = create_variables(args.clone());
                let outputs_ts_stream = create_outputs(args);
                quote! {
                    Element::Multiply(inputs) => {
                        if inputs.len() != #expected_elements {
                            return None;
                        }
                        #variables_ts_stream
                        #outputs_ts_stream
                    }
                }
            },
            MatchElement::Pow => {
                if operation_args.len() != 2 {
                    panic!("expected exactly two inner elements for pow");
                }
                let args = operation_args.iter().cloned().map(|a| a.into_iter().collect::<TokenStream2>());
                let var_ts_stream = create_variables(args.clone());
                let outputs_ts_stream = create_outputs(args);
                quote! {
                    Element::Pow(b, e) => {
                        let inputs = [b.as_ref(), e.as_ref()];
                        #var_ts_stream
                        #outputs_ts_stream
                    }
                }
            },
            MatchElement::Variable => {
                if operation_args.len() != 1 || operation_args[0].len() != 1 {
                    panic!("expected exactly one inner element for var");
                }
                if operation_args[0][0].to_string() != "x" {
                    panic!("expected identifier 'x' as inner element for var");
                }
                quote! { Element::Variable(n) => Some(n) }
            },
            MatchElement::Function => {
                let expected_elements = operation_args.len();
                let args = operation_args.iter().cloned().map(|a| a.into_iter().collect::<TokenStream2>());
                let variables_ts_stream = create_variables(args.clone());
                let outputs_ts_stream = create_outputs(args);
                quote! {
                    Element::Function { arguments: inputs, .. } => {
                        if inputs.len() != #expected_elements {
                            return None;
                        }
                        #variables_ts_stream
                        #outputs_ts_stream
                    }
                }
            },
            MatchElement::Negate => {
                if operation_args.len() != 1 {
                    panic!("expected exactly one inner element for neg");
                }
                let args = operation_args.iter().cloned().map(|a| a.into_iter().collect::<TokenStream2>());
                let variables_ts_stream = create_variables(args.clone());
                let outputs_ts_stream = create_outputs(args);
                quote! {
                    Element::Negate(element) => {
                        let inputs = [element.as_ref()];
                        #variables_ts_stream
                        #outputs_ts_stream
                    }
                }
            },
        }
    } else {
        let match_name = match_element.as_pattern();
        quote! {
            Element::#match_name {..} => Some(())
        }
    }
        .into();
    quote! {(||{
        let __expr = &#formula;
        match __expr {
            #match_stream,
            _ => None,
        }
    })()}
}

fn process_input(input: TokenStream2) -> MatchInput {
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
        Some(TokenTree::Ident(i)) => i.clone(),
        _ => panic!("expected identifier"),
    };
    let inner_elements = match_expr.get(1).map(|tt| match tt {
        TokenTree::Group(g) => split_by_comma(g.stream()),
        _ => panic!("unexpected token after identifier"),
    });
    MatchInput { formula: formula_ts, match_ident, inner_elements }
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

fn create_variables(ts: impl IntoIterator<Item=TokenStream2>) -> TokenStream2 {
    ts.into_iter()
        .enumerate()
        .map(|(i, ts)| {
            let var_name =
                TokenStream2::from(TokenTree::Ident(Ident::new(&create_var_name(i), Span::call_site())));
            if ts.to_string() == "x" {
                quote! {
                    let #var_name = inputs[#i];
                }
            } else if ts.to_string().contains("x") {
                quote! {
                    let #var_name = match_formula!(inputs[#i], #ts)?;
                }
            } else {
                quote! {
                    match_formula!(inputs[#i], #ts)?;
                }
            }
        })
        .collect()
}

fn create_var_name(i: usize) -> String {
    format!("var_{}", i)
}

fn create_outputs(ts: impl IntoIterator<Item=TokenStream2>) -> TokenStream2 {
    let string = ts
        .into_iter()
        .enumerate()
        .flat_map(|(i, x)| x.to_string().contains("x").then(|| create_var_name(i)))
        .collect::<Vec<_>>()
        .join(", ");
    TokenStream2::from_str(&format!("Some(({}))", string)).expect("could not create output")
}
