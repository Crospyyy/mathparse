use proc_macro::TokenStream;
use proc_macro2::{Ident, Span, TokenStream as TokenStream2, TokenTree};
use quote::{TokenStreamExt, quote};
use std::str::FromStr;

/// # Usage
/// ## Element matcher \[EM\]
/// - **Expression**
///     - `{some_expr}` Only match if the compared element is equal to the result of `some_expr`
///     - `{some_expr}..` Only match if all the compared elements are equal to the elements inside the array `some_expr`
/// - **Get Element**
///     - `x` Get the value of one Element
///     - `x..` Get the values of all the Elements as an array
///     - `x, x, x` Get the values of multiple Elements as an array
/// - **Match Any**
///     - `_` Match any element
/// - **Number**
///     - `num` Match any number element
///     - `num([NM])` Match the inner value of the number element
/// - **Variable**
///     - `var` Match any variable element
///     - `var([SM])` Match the inner value of the variable element
/// - **Negate**
///     - `neg` Match any negate element
///     - `neg([EM])` Match the inner element of the negate element
/// - **Plus**
///     - `plus` Match any plus element
///     - `plus([EM]..)` Match the elements inside the plus element
/// - **Multiply**
///     - `mul` Match any multiply element
///     - `mul([EM]..)` Match the elements inside the multiply element
/// - **Pow**
///     - `pow` Match any power element
///     - `pow([EM]..)` Match the base and exponent of the power element
/// - **Function**
///     - `fun` Match any function element
///     - `fun([SM], [EM]..)` Match the function name and the elements inside the function element
/// ## Number matcher \[NM\]
/// - **Get Number**
///     - `x` Get the value of one number element
/// - **Compare Number**
///     - `{some_expr}` Only match if the compared number is equal to the result of `some_expr`
/// ## String matcher \[SM\]
/// - **Get String**
///     - `x` get the value of one string element
/// - **Compare String**
///     - `{some_expr}` only match if the compared string is equal to the result of `some_expr`
#[proc_macro]
pub fn match_formula(item: TokenStream) -> TokenStream {
    let input: TokenStream2 = item.into();

	let MatchInput { formula, matcher } = MatchInput::parse(input);
	let MatchOutput { tokens, var_count } = generate_match(formula, matcher);
    let mut tokens = quote! { (||#tokens)() };
    if var_count == 0 {
        tokens.append_all(quote! { .is_some() })
    }
    tokens.into()
}

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

impl MatchInput {
	fn parse(input: TokenStream2) -> MatchInput {
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
}

fn parse_element_matcher(match_expr: TokenStream2) -> (Ident, Option<Vec<Vec<TokenTree>>>) {
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

struct MatchOutput {
    tokens: TokenStream2,
    var_count: usize,
}

fn generate_match(formula: TokenStream2, matcher: TokenStream2) -> MatchOutput {
	let (match_ident, inner_elements) = parse_element_matcher(matcher);
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
                        quote! { Element::Number(n) => (n == #l).then_some(()) }
                    },
                    _ => panic!("expected identifier or literal as inner element for num"),
                }
            },
            MatchElement::Plus => {
                let expected_elements = operation_args.len();
                let args = operation_args.iter().cloned().map(|a| a.into_iter().collect::<TokenStream2>());
                let VariablesOutput { tokens: variables_ts_stream, var_counts } =
					VariablesOutput::create(args.clone());
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
					VariablesOutput::create(args.clone());
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
					VariablesOutput::create(args.clone());
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
                        let inner_without_qoutes = inner[1..inner.len() - 1].to_string();
                        quote! { Element::Variable(n) => { (n == #inner_without_qoutes).then_some(()) } }
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
					VariablesOutput::create(args.clone());
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
					VariablesOutput::create(args.clone());
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
        match &#formula {
            #match_stream,
            _ => None,
        }
    }};
    MatchOutput { tokens, var_count }
}

struct VariablesOutput {
    tokens: TokenStream2,
    var_counts: Vec<usize>,
}

impl VariablesOutput {
	fn create(ts: impl IntoIterator<Item=TokenStream2>) -> VariablesOutput {
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
					let inner_call = generate_match(quote! {inputs[#i]}, ts.clone());
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
    (str.starts_with('"') && str.ends_with('"') | (str.starts_with('\'') && str.ends_with('\'')))
        && str.len() > 2
        && chars[1..chars.len() - 1].iter().all(|c| !"\"'".contains(*c))
}

fn outer(input: TokenStream) -> TokenStream {
	let new_stream = input.into();
	let MatchInput { formula, matcher } = MatchInput::parse(new_stream);
	let matcher = parse_match(matcher);
	let MatchOutput { tokens, var_count } = perform_match(formula, matcher);
	let mut tokens = quote! { (||#tokens)() };
	if var_count == 0 {
		tokens.append_all(quote! { .is_some() });
	}
	tokens.into()
}

fn parse_match(match_expr: TokenStream2) -> ElementMatcher {
	todo!()
}

fn perform_match(formula: TokenStream2, matcher: ElementMatcher) -> MatchOutput {
	todo!()
}

enum SingleOrMultipleElementMatcher {
	/// like `x` or `x, x`
	Single(Vec<ElementMatcher>),
	/// like `x..`
	Flatten(ElementMatcher),
}

enum ElementMatcher {
	Any,
	Number(Option<NumberMatcher>),
	Variable(Option<StringMatcher>),
	Negate(Option<Box<ElementMatcher>>),
	Plus(Option<Box<SingleOrMultipleElementMatcher>>),
	Multiply(Option<Box<SingleOrMultipleElementMatcher>>),
	Pow(Option<Box<SingleOrMultipleElementMatcher>>),
	Function(Option<(StringMatcher, Option<Box<SingleOrMultipleElementMatcher>>)>),
}

trait Matcher {
	fn perform_match(&self, formula: TokenStream2) -> MatchOutput;
}

impl Matcher for ElementMatcher {
	fn perform_match(&self, formula: TokenStream2) -> MatchOutput {
		todo!()
	}
}

enum NumberMatcher {
	GetValue,
	CompareTo(TokenStream2),
}

impl Matcher for NumberMatcher {
	fn perform_match(&self, formula: TokenStream2) -> MatchOutput {
		todo!()
	}
}

enum StringMatcher {
	GetValue,
	CompareTo(String),
}

impl Matcher for StringMatcher {
	fn perform_match(&self, formula: TokenStream2) -> MatchOutput {
		todo!()
	}
}
