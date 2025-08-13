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
    matcher: TokenStream2,
}

#[proc_macro]
pub fn match_formula(item: TokenStream) -> TokenStream {
    let input: TokenStream2 = item.into();

    let MatchInput { formula, matcher } = process_input(input);
    let MatchOutput { tokens, var_count } = create_code(formula, matcher);
    if var_count == 0 {
        quote! {
            (||{
                #tokens
            })().is_some()
        }
            .into()
    } else {
        quote! {
            (||{
                #tokens
            })()
        }
            .into()
    }
}

struct MatchOutput {
    tokens: TokenStream2,
    var_count: usize,
}

fn create_code(formula: TokenStream2, matcher: TokenStream2) -> MatchOutput {
    let (match_ident, inner_elements) = generate_match_variables(matcher);
    if match_ident.to_string() == "_" && inner_elements.is_none() {
        return MatchOutput { tokens: quote! { Some(()) }, var_count: 0 };
    }

    let mut var_count = 0usize;

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
                        var_count += 1;
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
                let VariablesOutput { tokens: variables_ts_stream, var_counts } =
                    create_variables(args.clone());
                let outputs_ts_stream = create_outputs(&var_counts);
                var_count += var_counts.iter().copied().sum::<usize>();
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
                let VariablesOutput { tokens: variables_ts_stream, var_counts } =
                    create_variables(args.clone());
                let outputs_ts_stream = create_outputs(&var_counts);
                var_count += var_counts.iter().copied().sum::<usize>();
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
                let VariablesOutput { tokens: variables_ts_stream, var_counts } =
                    create_variables(args.clone());
                let outputs_ts_stream = create_outputs(&var_counts);
                var_count += var_counts.iter().copied().sum::<usize>();
                quote! {
                    Element::Pow(b, e) => {
                        let inputs = [b.as_ref(), e.as_ref()];
                        #variables_ts_stream
                        #outputs_ts_stream
                    }
                }
            },
            MatchElement::Variable => {
                if operation_args.len() != 1 {
                    panic!("expected exactly one inner element for var");
                }
                let inner = operation_args[0].iter().map(|e| e.to_string()).collect::<String>();
                if inner == "x" {
                    var_count += 1;
                    quote! { Element::Variable(n) => Some(n) }
                } else {
                    if is_in_quotes(&inner) {
                        quote! { Element::Variable(n) => { (n == #inner).then_some(()) } }
                    } else {
                        panic!(
                            "expected identifier 'x' or a string literal as inner element for var, got '{}'",
                            inner
                        );
                    }
                }
            },
            MatchElement::Function => {
                let expected_elements = operation_args.len();
                let args = operation_args.iter().cloned().map(|a| a.into_iter().collect::<TokenStream2>());
                let VariablesOutput { tokens: variables_ts_stream, var_counts } =
                    create_variables(args.clone());
                let outputs_ts_stream = create_outputs(&var_counts);
                var_count += var_counts.iter().copied().sum::<usize>();

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
                let VariablesOutput { tokens: variables_ts_stream, var_counts } =
                    create_variables(args.clone());
                let outputs_ts_stream = create_outputs(&var_counts);
                var_count += var_counts.iter().copied().sum::<usize>();

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
    let tokens = quote! {{
        let __expr = &#formula;
        match __expr {
            #match_stream,
            _ => None,
        }
    }};
    MatchOutput { tokens, var_count }
}

fn process_input(input: TokenStream2) -> MatchInput {
    let input_args = split_by_comma_2(input);
    if input_args.len() != 2 {
        panic!("expected exactly two arguments");
    }
    let mut input_args_iter = input_args.into_iter();

    let formula = input_args_iter.next().unwrap();
    let match_expr = input_args_iter.next().unwrap();

    let formula_ts: TokenStream2 = formula.into_iter().collect();

    MatchInput { formula: formula_ts, matcher: match_expr }
}

fn generate_match_variables(match_expr: TokenStream2) -> (Ident, Option<Vec<Vec<TokenTree>>>) {
    let match_expr = match_expr.into_iter().collect::<Vec<_>>();
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
    (match_ident, inner_elements)
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
fn split_by_comma_2(ts: TokenStream2) -> Vec<TokenStream2> {
    // initialisiere ergebnisvektor
    let mut result = Vec::new();
    // sammle tokens bis zum kommatrennzeichen
    let mut segment = Vec::new();
    for tt in ts.into_iter() {
        if let TokenTree::Punct(p) = &tt {
            if p.as_char() == ',' {
                result.push(segment.into_iter().collect());
                segment = Vec::new();
                continue;
            }
        }
        segment.push(tt);
    }
    // letztes segment hinzufügen wenn nicht leer
    if !segment.is_empty() {
        result.push(segment.into_iter().collect());
    }
    result
}

struct VariablesOutput {
    tokens: TokenStream2,
    var_counts: Vec<usize>,
}

fn create_variables(ts: impl IntoIterator<Item=TokenStream2>) -> VariablesOutput {
    let mut var_counts = Vec::new();
    let tokens = ts
        .into_iter()
        .enumerate()
        .map(|(i, ts)| {
            let var_name =
                TokenStream2::from(TokenTree::Ident(Ident::new(&create_var_name(i), Span::call_site())));
            if ts.to_string() == "x" {
                var_counts.push(1);
                quote! {
                    let #var_name = inputs[#i];
                }
            } else {
                let inner_call = create_code(quote! {inputs[#i]}, ts.clone());
                var_counts.push(inner_call.var_count);
                let tokens_inner_call = inner_call.tokens;
                if inner_call.var_count != 0 {
                    quote! {
                        let #var_name = #tokens_inner_call?;
                    }
                } else {
                    quote! {
                        #tokens_inner_call?;
                    }
                }
            }
        })
        .collect();
    VariablesOutput { tokens, var_counts }
}

fn create_var_name(i: usize) -> String {
    format!("var_{}", i)
}

fn create_flattened_var(name: &str, count: usize) -> String {
    (0..count).map(|i| format!("{}.{}", name, i)).collect::<Vec<_>>().join(", ")
}

fn create_outputs(var_counts: &[usize]) -> TokenStream2 {
    let string = var_counts
        .iter()
        .copied()
        .enumerate()
        .flat_map(|(i, var_count)| {
            (var_count != 0).then(|| {
                let name = create_var_name(i);
                if var_count == 1 { name } else { create_flattened_var(&name, var_count) }
            })
        })
        .collect::<Vec<_>>()
        .join(", ");
    TokenStream2::from_str(&format!("Some(({}))", string)).expect("could not create output")
}

fn is_in_quotes(str: &String) -> bool {
    let chars = str.chars().collect::<Vec<_>>();
    (str.starts_with('"') && str.ends_with('"') | str.starts_with('\'') && str.ends_with('\''))
        && str.len() > 2
        && chars[1..chars.len() - 1].iter().all(|c| !"\"'".contains(*c))
}
