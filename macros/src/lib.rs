use proc_macro::TokenStream;
use quote::quote;

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
pub fn formula_matches(item: TokenStream) -> TokenStream {
	new::outer(item)
}

#[proc_macro]
pub fn return_tokens(item: TokenStream) -> TokenStream {
	let strings: Vec<_> = item.into_iter().map(|e| format!("{:?}", e)).collect();
	let string = strings.join(", ");
	quote! {#string}.into()
}

mod new {
	use proc_macro::TokenStream as TokenStreamOld;
	use proc_macro2::{Ident, Span, TokenStream, TokenTree};
	use quote::{TokenStreamExt, quote};
	use std::str::FromStr;

	pub(super) struct MatchInput {
		pub(crate) formula: TokenStream,
		pub(crate) matcher: TokenStream,
	}

	impl MatchInput {
		pub(super) fn parse(input: TokenStream) -> MatchInput {
			let input_args = split_by_comma_2(input);
			if input_args.len() != 2 {
				panic!("expected exactly two arguments");
			}
			let mut input_args_iter = input_args.into_iter();

			let formula = input_args_iter.next().unwrap();
			let match_expr = input_args_iter.next().unwrap();

			let formula_ts: TokenStream = formula.into_iter().collect();

			MatchInput { formula: formula_ts, matcher: match_expr }
		}
	}

	fn create_flattened_var(name: &str, count: usize) -> String {
		(0..count).map(|i| format!("{}.{}", name, i)).collect::<Vec<_>>().join(", ")
	}

	pub(super) fn create_outputs(var_counts: &[usize]) -> TokenStream {
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
		TokenStream::from_str(&format!("Some(({}))", string)).expect("could not create output")
	}

	pub(super) fn create_var_name(i: usize) -> String {
		format!("__var_{}", i)
	}

	pub(super) struct MatchOutput {
		pub(crate) tokens: TokenStream,
		pub(crate) var_count: usize,
	}

	impl MatchOutput {
		fn no_output(tokens: TokenStream) -> Self {
			Self { tokens, var_count: 0 }
		}

		fn with_output(tokens: TokenStream, var_count: usize) -> Self {
			Self { tokens, var_count }
		}

		fn generate_variable(&self, index: usize) -> TokenStream {
			let tokens = &self.tokens;
			if self.var_count != 0 {
				let var_name = TokenStream::from_str(&create_var_name(index)).unwrap();
				quote! { let #var_name = #tokens?; }
			} else {
				quote! { #tokens?; }
			}
		}

		fn create_code(matches: &[ElementMatcher], formula: TokenStream, use_reference: bool) -> MatchOutput {
			let match_tokens: Vec<_> = matches
				.iter()
				.enumerate()
				.map(|(i, m)| {
					m.perform_match(if use_reference {
						quote! { &#formula[#i] }
					} else {
						quote! { #formula[#i] }
					})
				})
				.collect();
			let variables =
				match_tokens.iter().enumerate().map(|(i, m)| m.generate_variable(i)).collect::<TokenStream>();
			let var_count = match_tokens.iter().map(|m| m.var_count).sum();
			let outputs = create_outputs(&match_tokens.iter().map(|m| m.var_count).collect::<Vec<_>>());
			MatchOutput::with_output(
				quote! {
					#variables
					#outputs
				},
				var_count,
			)
		}
	}

	pub enum MatchElement {
		Number,
		Negate,
		Plus,
		Multiply,
		Pow,
		Variable,
		Function,
	}

	impl MatchElement {
		pub(crate) fn from_str(s: &str) -> Option<Self> {
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
		
		pub(crate) fn as_pattern(&self) -> TokenTree {
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

	enum SingleOrMultipleElementMatcher {
		/// like `x` or `x, x`
		EachMatch(Vec<ElementMatcher>),
		/// like `x..`
		Flatten(ElementMatcher),
	}

	enum ElementMatcher {
		/// _
		Any,
		/// x
		X,
		/// some_expr
		Expression(TokenStream),
		/// num / pow / plus
		WithoutInner(MatchElement),
		Number(NumberMatcher),
		Variable(StringMatcher),
		Negate(Box<ElementMatcher>),
		Plus(Box<SingleOrMultipleElementMatcher>),
		Multiply(Box<SingleOrMultipleElementMatcher>),
		Pow(Box<SingleOrMultipleElementMatcher>),
		Function(StringMatcher, Box<SingleOrMultipleElementMatcher>),
	}

	trait Matcher {
		fn perform_match(&self, formula: TokenStream) -> MatchOutput;
	}

	impl Matcher for ElementMatcher {
		fn perform_match(&self, formula: TokenStream) -> MatchOutput {
			match self {
				ElementMatcher::Any => MatchOutput::no_output(quote! { Some(()) }),
				ElementMatcher::X => MatchOutput::with_output(quote! { Some(#formula) }, 1),
				ElementMatcher::Expression(expr) => {
					MatchOutput::no_output(quote! { ((#formula) == (#expr)).then_some(()) })
				},
				ElementMatcher::WithoutInner(element) => {
					let element_string = element.as_pattern();
					MatchOutput::no_output(
						quote! { matches!(#formula, Element::#element_string {..}).then_some(()) },
					)
				},
				ElementMatcher::Number(n) => {
					let inner = n.perform_match(quote! { n });
					let inner_tokens = inner.tokens;
					MatchOutput::with_output(
						quote! { if let Element::Number(n) = #formula { #inner_tokens } else { None } },
						inner.var_count,
					)
				},
				ElementMatcher::Variable(n) => {
					let inner = n.perform_match(quote! { n });
					let inner_tokens = inner.tokens;
					MatchOutput::with_output(
						quote! { if let Element::Variable(n) = #formula { #inner_tokens } else { None } },
						inner.var_count,
					)
				},
				ElementMatcher::Negate(n) => {
					let inner = n.perform_match(quote! { n.as_ref() });
					let inner_tokens = inner.tokens;
					MatchOutput::with_output(
						quote! { if let Element::Negate(n) = #formula { #inner_tokens } else { None } },
						inner.var_count,
					)
				},
				ElementMatcher::Plus(n) => {
					let inner = n.as_ref().perform_match(quote! { inputs }, true);
					let inner_tokens = inner.tokens;
					MatchOutput::with_output(
						quote! { if let Element::Plus(inputs) = #formula { #inner_tokens } else { None } },
						inner.var_count,
					)
				},
				ElementMatcher::Multiply(n) => {
					let inner = n.as_ref().perform_match(quote! { inputs }, true);
					let inner_tokens = inner.tokens;
					MatchOutput::with_output(
						quote! { if let Element::Multiply(inputs) = #formula { #inner_tokens } else { None } },
						inner.var_count,
					)
				},
				ElementMatcher::Pow(n) => {
					let inner = n.as_ref().perform_match(quote! { inputs }, false);
					let inner_tokens = inner.tokens;
					MatchOutput::with_output(
						quote! {
							if let Element::Pow(__b, __e) = #formula {
								let inputs = [__b.as_ref(), __e.as_ref()];
								#inner_tokens
							} else { None }
						},
						inner.var_count,
					)
				},
				ElementMatcher::Function(s, n) => {
					let name_inner = s.perform_match(quote! { name });
					let args_inner = n.perform_match(quote! { arguments }, true);
					let name_var = name_inner.generate_variable(0);
					let args_var = args_inner.generate_variable(1);
					let outputs = create_outputs(&[name_inner.var_count, args_inner.var_count]);

					MatchOutput::with_output(
						quote! {
						if let Element::Function { name, arguments } = #formula {
							#name_var
							#args_var
							#outputs
						} else { None } },
						name_inner.var_count + args_inner.var_count,
					)
				},
			}
		}
	}

	impl SingleOrMultipleElementMatcher {
		fn perform_match(&self, formula: TokenStream, use_reference: bool) -> MatchOutput {
			match self {
				SingleOrMultipleElementMatcher::EachMatch(matches) => {
					MatchOutput::create_code(matches, formula, use_reference)
				},
				SingleOrMultipleElementMatcher::Flatten(matcher) => {
					let inner = matcher.perform_match(quote! { e });
					let tokens = inner.tokens;
					MatchOutput::with_output(
						if use_reference {
							quote! {
								#formula.iter().map(|e| #tokens).collect::<Option<Vec<_>>>()
							}
						} else {
							quote! {
								#formula.into_iter().map(|e| #tokens).collect::<Option<Vec<_>>>()
							}
						},
						1,
					)
				},
			}
		}
	}

	enum NumberMatcher {
		GetValue,
		CompareTo(TokenStream),
	}

	impl Matcher for NumberMatcher {
		fn perform_match(&self, formula: TokenStream) -> MatchOutput {
			match self {
				NumberMatcher::GetValue => MatchOutput::with_output(quote! { Some(#formula) }, 1),
				NumberMatcher::CompareTo(comp_val) => MatchOutput::no_output(quote! {
					(#formula == (#comp_val)).then_some(())
				}),
			}
		}
	}

	enum StringMatcher {
		GetValue,
		CompareTo(TokenStream),
	}

	impl Matcher for StringMatcher {
		fn perform_match(&self, formula: TokenStream) -> MatchOutput {
			match self {
				StringMatcher::GetValue => MatchOutput::with_output(quote! { Some(#formula) }, 1),
				StringMatcher::CompareTo(comp_val) => MatchOutput::no_output(quote! {
					(#formula == #comp_val).then_some(())
				}),
			}
		}
	}

	pub(super) fn split_by_comma_2(ts: TokenStream) -> Vec<TokenStream> {
		// initialisiere ergebnisvektor
		let mut result = Vec::new();
		// sammle tokens bis zum kommatrennzeichen
		let mut segment = Vec::new();
		for tt in ts.into_iter() {
			if let TokenTree::Punct(p) = &tt
				&& p.as_char() == ','
			{
				result.push(segment.into_iter().collect());
				segment = Vec::new();
				continue;
			}
			segment.push(tt);
		}
		// letztes segment hinzufügen wenn nicht leer
		if !segment.is_empty() {
			result.push(segment.into_iter().collect());
		}
		result
	}

	fn parse_element_matcher(match_expr: TokenStream) -> (Ident, Option<TokenStream>) {
		let match_expr = match_expr.into_iter().collect::<Vec<_>>();
		// ident
		if match_expr.len() > 2 {
			panic!("Expected no more than two tokens in match expression");
		}
		let match_ident = match match_expr.first() {
			Some(TokenTree::Ident(i)) => i.clone(),
			_ => panic!("expected identifier"),
		};
		let inner_elements = match_expr.get(1).map(|tt| match tt {
			TokenTree::Group(g) => g.stream(),
			_ => panic!("unexpected token after identifier"),
		});
		(match_ident, inner_elements)
	}

	trait MatchParsing {
		fn parse(match_expr: TokenStream) -> Self;
	}

	impl MatchParsing for ElementMatcher {
		fn parse(match_expr: TokenStream) -> ElementMatcher {
			let string = match_expr.to_string();
			if string == "_" {
				return ElementMatcher::Any;
			}
			if string == "x" {
				return ElementMatcher::X;
			}
			if string.starts_with("{") && string.ends_with("}") {
				let mut tokens = match_expr.clone().into_iter().collect::<Vec<_>>();
				if tokens.len() != 1 {
					panic!("expected exactly one token inside braces, got {}", tokens.len());
				}
				let group = tokens.pop().unwrap();
				let TokenTree::Group(g) = group else { panic!("expected a group inside braces") };
				return ElementMatcher::Expression(g.stream());
			}
			let (match_ident, inner_elements) = parse_element_matcher(match_expr);
			let match_ident_str = match_ident.to_string();
			let match_element =
				MatchElement::from_str(&match_ident_str).expect("unexpected element to match on");
			if let Some(inner) = inner_elements {
				match match_element {
					MatchElement::Number => ElementMatcher::Number(NumberMatcher::parse(inner)),
					MatchElement::Negate => ElementMatcher::Negate(Box::new(ElementMatcher::parse(inner))),
					MatchElement::Plus => {
						ElementMatcher::Plus(Box::new(SingleOrMultipleElementMatcher::parse(inner)))
					},
					MatchElement::Multiply => {
						ElementMatcher::Multiply(Box::new(SingleOrMultipleElementMatcher::parse(inner)))
					},
					MatchElement::Pow => {
						ElementMatcher::Pow(Box::new(SingleOrMultipleElementMatcher::parse(inner)))
					},
					MatchElement::Variable => ElementMatcher::Variable(StringMatcher::parse(inner)),
					MatchElement::Function => {
						let split = split_by_comma_2(inner);
						if split.len() != 2 {
							panic!("expected exactly two inner elements for function matcher");
						}
						ElementMatcher::Function(
							StringMatcher::parse(split[0].clone()),
							Box::new(SingleOrMultipleElementMatcher::parse(split[1].clone())),
						)
					},
				}
			} else {
				ElementMatcher::WithoutInner(match_element)
			}
		}
	}

	impl MatchParsing for NumberMatcher {
		fn parse(match_expr: TokenStream) -> NumberMatcher {
			if match_expr.to_string() == "x" {
				NumberMatcher::GetValue
			} else {
				NumberMatcher::CompareTo(match_expr)
			}
		}
	}

	impl MatchParsing for StringMatcher {
		fn parse(match_expr: TokenStream) -> Self {
			if match_expr.to_string() == "x" {
				StringMatcher::GetValue
			} else {
				StringMatcher::CompareTo(match_expr)
			}
		}
	}

	impl MatchParsing for SingleOrMultipleElementMatcher {
		fn parse(match_expr: TokenStream) -> Self {
			let split = split_by_comma_2(match_expr.clone());
			match split.len() {
				0 => panic!("no token provided for matching"),
				1 => {
					let arr: Vec<_> = match_expr.clone().into_iter().collect();
					if match_expr.to_string().ends_with("..") {
						let tokens: TokenStream = arr[..(arr.len() - 2)].iter().cloned().collect();
						SingleOrMultipleElementMatcher::Flatten(ElementMatcher::parse(tokens))
					} else {
						SingleOrMultipleElementMatcher::EachMatch(vec![ElementMatcher::parse(match_expr)])
					}
				},
				2.. => SingleOrMultipleElementMatcher::EachMatch(
					split.into_iter().map(ElementMatcher::parse).collect(),
				),
			}
		}
	}

	pub(super) fn outer(input: TokenStreamOld) -> TokenStreamOld {
		let new_stream = input.into();

		let MatchInput { formula, matcher } = MatchInput::parse(new_stream);
		let matcher = ElementMatcher::parse(matcher);
		let MatchOutput { tokens, var_count } = matcher.perform_match(quote! { &#formula });
		let mut tokens = quote! { (||#tokens)() };
		if var_count == 0 {
			tokens.append_all(quote! { .is_some() });
		}

		tokens.into()
	}
}
