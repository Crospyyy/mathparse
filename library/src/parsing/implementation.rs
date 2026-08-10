use crate::benchmarking::Benchmark;
use crate::{Element, ElementParsed, Number};
use regex::Regex;
use std::mem;
use std::sync::LazyLock;
use thiserror::Error;

static REGEX_INSERT_PLUS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([\w)])-([\w(-])").unwrap());

fn is_valid_char_for_function_name(c: char) -> bool {
	matches!(c, 'a'..='z' | 'A'..='Z' | '_' | '0'..='9')
}

pub fn get_fun_name_end_of_string(name: &str, allow_first_char_digit: bool) -> String {
	let mut valid_chars_count = 0;
	name.chars()
		.rev()
		.take_while(|&c| is_valid_char_for_function_name(c))
		.for_each(|_| valid_chars_count += 1);
	if valid_chars_count == 0 {
		"".to_owned()
	} else {
		let name = name[name.len() - valid_chars_count..].to_owned();
		if allow_first_char_digit || name.chars().next().is_some_and(|c| !c.is_ascii_digit()) {
			name
		} else {
			"".to_owned()
		}
	}
}

#[derive(Error, Debug)]
pub enum ParseError {
	#[error("Error processing power: {0}")]
	ProcessingPow(String),
	#[error("Unparsed element left: {0:?}")]
	UnparsedElementLeft(Element),
}

impl Element {
	pub fn parse(input: &str) -> Result<ElementParsed, ParseError> {
		Self::parse_benched(input, &mut Benchmark::new("Parse formula"))
	}

	pub fn parse_benched(input: &str, benchmark: &mut Benchmark) -> Result<ElementParsed, ParseError> {
		let b = benchmark;

		let cow = b.benchmark("preprocess_string_minus", || Element::preprocess_string(input));
		let chars = b.benchmark("convert_to_chars", || cow.chars().collect::<Vec<_>>());
		let mut start = 0;
		let mut formula = b.benchmark("resolve_brackets", || Element::resolve_brackets(&chars, &mut start));
		b.benchmark("resolve_functions", || formula.resolve_functions());
		b.benchmark("process_plus", || formula.process_plus());
		b.benchmark("process_minus", || formula.process_minus());
		b.benchmark("process_multiply", || formula.process_multiply());
		b.benchmark("process_divide", || formula.process_divide());
		b.benchmark("process_minus_2", || formula.process_minus());
		b.benchmark("process_pow", || formula.process_pow()).map_err(ParseError::ProcessingPow)?;
		b.benchmark("process_minus_3", || formula.process_minus());
		b.benchmark("process_numbers_and_variables", || formula.process_numbers_and_variables());
		b.benchmark("remove_unneeded_outer_brackets", || formula.remove_unneeded_outer_brackets());
		b.benchmark("convert_to_variables_where_possible", || formula.convert_to_variables_where_possible());
		Ok(formula.try_into().map_err(ParseError::UnparsedElementLeft)?)
	}

	/// Step 0
	pub(crate) fn preprocess_string(input: &str) -> String {
		let without_whitespace = input.replace(' ', "").replace('×', "*");
		let mut modified = without_whitespace;
		loop {
			let new = REGEX_INSERT_PLUS.replace_all(&modified, "$1+-$2").to_string();
			if new == modified {
				break;
			} else {
				modified = new;
			}
		}
		modified
	}

	/// Step 1
	pub(crate) fn resolve_brackets(input: &[char], start: &mut usize) -> Element {
		let mut elements = Vec::new();
		let mut i = *start;
		while i < input.len() {
			let char = input[i];
			if !"()".contains(char) {
				i += 1;
				if i == input.len() && *start < i {
					elements.push(Element::String(input[*start..i].iter().collect()));
				}
				continue;
			}
			if char == '(' {
				if i > *start {
					elements.push(Element::String(input[*start..i].iter().collect()));
				}
				i += 1; // ensure the pointer is behind the opening brackets
				elements.push(Self::resolve_brackets(input, &mut i));
				*start = i;
			}
			if char == ')' {
				if i != *start {
					elements.push(Element::String(input[*start..i].iter().collect()));
				}
				i += 1; // ensure that the pointer is behind the closing brackets
				*start = i;
				return Element::Brackets(elements);
			}
		}
		*start = i;
		Element::Brackets(elements)
	}

	/// Step 0,5
	pub(crate) fn resolve_functions(&mut self) {
		match self {
			Element::Brackets(elements) => {
				if elements.is_empty() {
					return;
				}
				for i in (0..elements.len() - 1).rev() {
					let j = i + 1;
					if let (Element::String(name), Element::Brackets(br_elements)) =
						(&elements[i], &elements[j])
					{
						let name = name.to_string();
						let function_name = get_fun_name_end_of_string(&name, false);
						if function_name.is_empty() {
							continue; // Function names cannot be empty or start with a digit
						}

						// create the new function element
						let arguments = split_list_by_char(br_elements, ',')
							.unwrap_or_else(|| vec![Element::Brackets(br_elements.clone())]);

						elements[j] = Element::Function { name: function_name.clone(), arguments };

						// update or remove the string element
						let new_str_len = name.len() - function_name.len();
						if new_str_len == 0 {
							elements.remove(i);
						} else {
							elements[i] = Element::String(name[..new_str_len].to_owned());
						}
					}
				}
				elements.iter_mut().for_each(Element::resolve_functions);
			},
			Element::Plus(elements)
			| Element::Multiply(elements)
			| Element::Function { arguments: elements, .. } => {
				elements.iter_mut().for_each(Element::resolve_functions);
			},
			Element::Negate(element) => element.resolve_functions(),
			Element::Pow(base, exponent) => {
				base.resolve_functions();
				exponent.resolve_functions();
			},
			_ => {},
		}
	}

	/// Step 2
	pub(crate) fn process_plus(&mut self) {
		match self {
			Element::Brackets(elements) => {
				if let Some(elements) = split_list_by_char(elements, '+') {
					*self = Element::Plus(elements);
				}
				if let Element::Brackets(elements) | Element::Plus(elements) = self {
					elements.iter_mut().for_each(Element::process_plus);
				}
			},
			Element::Function { arguments, .. } => {
				arguments.iter_mut().for_each(Element::process_plus);
			},
			Element::String(s) => {
				if let Some(elements) = split_string_by_char(s, '+') {
					*self = Element::Plus(elements);
				}
			},
			Element::Plus(_)
			| Element::Multiply(_)
			| Element::Negate(_)
			| Element::Variable(_)
			| Element::VariableOrFunction(_)
			| Element::Number(_)
			| Element::FunctionWithExpression { .. }
			| Element::NumberWithExpression { .. }
			| Element::Pow(..) => {},
		}
	}

	/// Steps 3, 5 and 7
	pub(crate) fn process_minus(&mut self) {
		match self {
			Element::Brackets(elements) => {
				if let Some(Element::String(str)) = elements.first() {
					if let Some(trimmed) = str.strip_prefix('-') {
						let new_string = trimmed.to_owned();
						if new_string.is_empty() {
							elements.remove(0);
						} else {
							elements[0] = Element::String(new_string);
						}
						*self = Element::Negate(Box::new(self.clone()));
					}
				} else if elements.len() == 1 {
					elements[0].process_minus();
				}
				if let Element::Brackets(groups) = self {
					groups.iter_mut().for_each(|e| match e {
						Element::String(_) => {},
						_ => e.process_minus(),
					})
				}
			},
			Element::Plus(elements) => {
				elements.iter_mut().for_each(Element::process_minus);
			},
			Element::Multiply(elements) => {
				elements.iter_mut().for_each(Element::process_minus);
			},
			Element::String(s) => {
				if let Some(stripped) = s.strip_prefix('-') {
					*self = Element::Negate(Box::new(Element::String(stripped.to_owned())));
					self.process_minus();
				}
			},
			Element::Negate(element) => {
				element.process_minus();
			},
			Element::Pow(b, e) => {
				b.process_minus();
				e.process_minus();
			},
			Element::Function { arguments, .. } => arguments.iter_mut().for_each(Element::process_minus),
			Element::Variable(_)
			| Element::Number(_)
			| Element::VariableOrFunction(_)
			| Element::FunctionWithExpression { .. }
			| Element::NumberWithExpression { .. } => {},
		}
	}

	/// Step 3
	pub(crate) fn process_multiply(&mut self) {
		match self {
			Element::Brackets(elements) => {
				if let Some(groups) = split_list_by_char(elements, '*') {
					*self = Element::Multiply(groups);
				}
				if let Element::Brackets(elements) | Element::Multiply(elements) = self {
					elements.iter_mut().for_each(Element::process_multiply);
				}
			},
			Element::Plus(elements) => elements.iter_mut().for_each(Element::process_multiply),
			Element::Negate(element) => element.process_multiply(),
			Element::String(s) => {
				if let Some(elements) = split_string_by_char(s, '*') {
					*self = Element::Multiply(elements);
				}
			},
			Element::Function { arguments, .. } => arguments.iter_mut().for_each(Element::process_multiply),
			Element::Multiply(_)
			| Element::Variable(_)
			| Element::Number(_)
			| Element::VariableOrFunction(_)
			| Element::Pow(_, _)
			| Element::FunctionWithExpression { .. }
			| Element::NumberWithExpression { .. } => {},
		}
	}

	/// Used in process_divide (Step 4)
	fn invert(&mut self) {
		*self = Element::Pow(
			Box::new(self.clone()),
			Box::new(Element::Negate(Box::new(Element::String("1".to_owned())))),
		);
	}

	/// Step 4
	pub(crate) fn process_divide(&mut self) {
		let create_divisions = |element: &mut Element, mut new_elements: Vec<Element>| {
			new_elements[1..].iter_mut().for_each(Element::invert);
			*element = Element::Multiply(new_elements);
		};
		match self {
			Element::String(str) => {
				if let Some(new_elements) = split_string_by_char(str, '/') {
					create_divisions(self, new_elements);
				}
			},
			Element::Brackets(elements) => {
				if let Some(new_elements) = split_list_by_char(elements, '/') {
					create_divisions(self, new_elements);
				}
				if let Element::Brackets(elements) | Element::Multiply(elements) = self {
					elements.iter_mut().for_each(Element::process_divide);
				}
			},
			Element::Plus(elements) => {
				elements.iter_mut().for_each(Element::process_divide);
			},
			Element::Multiply(elements) => {
				elements.iter_mut().for_each(Element::process_divide);
			},
			Element::Function { arguments, .. } => {
				arguments.iter_mut().for_each(Element::process_divide);
			},
			Element::Negate(e) => e.process_divide(),
			Element::Pow(a, b) => {
				a.process_divide();
				b.process_divide();
			},
			Element::Variable(_)
			| Element::Number(_)
			| Element::VariableOrFunction(_)
			| Element::FunctionWithExpression { .. }
			| Element::NumberWithExpression { .. } => {},
		}
	}

	/// Step 6
	pub(crate) fn process_pow(&mut self) -> Result<(), String> {
		let create_recursive_pow = |element: &mut Element,
		                            mut new_elements: Vec<Element>|
		 -> Result<(), String> {
			let mut working_element = new_elements.pop().ok_or("No element provided for power operation")?;
			for e in new_elements.into_iter().rev() {
				working_element = Element::Pow(Box::new(e), Box::new(working_element));
			}
			*element = working_element;
			Ok(())
		};
		match self {
			Element::Brackets(elements) => {
				if let Some(new_elements) = split_list_by_char(elements, '^') {
					create_recursive_pow(self, new_elements)?;
				}
				match self {
					Element::Brackets(elements) => elements.iter_mut().try_for_each(Element::process_pow),
					Element::Pow(b, e) => {
						b.process_pow()?;
						e.process_pow()
					},
					_ => Ok(()),
				}
			},
			Element::Plus(elements) | Element::Multiply(elements) => {
				elements.iter_mut().try_for_each(Element::process_pow)
			},
			Element::Negate(e) => e.process_pow(),
			Element::String(s) => {
				if let Some(new_elements) = split_string_by_char(s, '^') {
					create_recursive_pow(self, new_elements)
				} else {
					Ok(())
				}
			},
			Element::Pow(b, p) => {
				b.process_pow()?;
				p.process_pow()
			},
			Element::Function { arguments, .. } => arguments.iter_mut().try_for_each(Element::process_pow),
			Element::Variable(_)
			| Element::Number(_)
			| Element::VariableOrFunction(_)
			| Element::FunctionWithExpression { .. }
			| Element::NumberWithExpression { .. } => Ok(()),
		}
	}

	/// Step 8
	pub(crate) fn process_numbers_and_variables(&mut self) {
		match self {
			Element::String(s) => {
				if let Some(num) = Number::from_string(&s) {
					*self = Element::Number(num);
				} else if s.chars().all(is_valid_char_for_function_name) {
					// If parsing fails, we assume it's a variable or function
					*self = Element::VariableOrFunction(s.clone());
				}
			},
			Element::Brackets(e) | Element::Multiply(e) | Element::Plus(e) => {
				e.iter_mut().for_each(Element::process_numbers_and_variables);
			},
			Element::Negate(e) => e.process_numbers_and_variables(),
			Element::Pow(base, exponent) => {
				base.process_numbers_and_variables();
				exponent.process_numbers_and_variables();
			},
			Element::Function { arguments, .. } => {
				arguments.iter_mut().for_each(|e| {
					if let Element::String(s) = e {
						if let Some(num) = Number::from_string(&s) {
							*e = Element::Number(num);
						} else {
							*e = Element::VariableOrFunction(s.clone());
						}
					} else {
						e.process_numbers_and_variables();
					}
				});
			},
			Element::Variable(_)
			| Element::Number(_)
			| Element::VariableOrFunction(_)
			| Element::FunctionWithExpression { .. }
			| Element::NumberWithExpression { .. } => {},
		}
	}

	/// Step 9
	pub(crate) fn remove_unneeded_outer_brackets(&mut self) {
		match self {
			Element::Brackets(elements) => {
				elements.iter_mut().for_each(Element::remove_unneeded_outer_brackets);
				if elements.len() == 1 {
					*self = elements.remove(0);
				}
			},
			Element::Plus(elements)
			| Element::Multiply(elements)
			| Element::Function { arguments: elements, .. } => {
				elements.iter_mut().for_each(Element::remove_unneeded_outer_brackets);
				elements.retain(|e| *e != Element::Brackets(vec![]))
			},
			Element::Pow(base, exponent) => {
				base.remove_unneeded_outer_brackets();
				exponent.remove_unneeded_outer_brackets();
			},
			Element::Negate(element) => element.remove_unneeded_outer_brackets(),
			Element::Variable(_)
			| Element::Number(_)
			| Element::String(_)
			| Element::VariableOrFunction(_)
			| Element::FunctionWithExpression { .. }
			| Element::NumberWithExpression { .. } => {},
		}
	}

	/// Step 10
	pub(crate) fn convert_to_variables_where_possible(&mut self) {
		match self {
			Element::String(_) | Element::Brackets(_) => {}, // these shouldn't exist at this point
			Element::Plus(elements) | Element::Multiply(elements) => {
				for arg in elements {
					if !arg.try_convert_to_variable() {
						arg.convert_to_variables_where_possible();
					}
				}
			},
			Element::Function { arguments, .. } => {
				arguments.iter_mut().for_each(Element::convert_to_variables_where_possible);
			},
			Element::Pow(a, b) => {
				if !a.try_convert_to_variable() {
					a.convert_to_variables_where_possible();
				}
				if !b.try_convert_to_variable() {
					b.convert_to_variables_where_possible();
				}
			},
			Element::Negate(x) => {
				if !x.try_convert_to_variable() {
					x.convert_to_variables_where_possible();
				}
			},
			Element::FunctionWithExpression { arguments, .. } => {
				arguments.iter_mut().for_each(|e| {
					if !e.try_convert_to_variable() {
						e.convert_to_variables_where_possible();
					}
				});
			},
			Element::Variable(_)
			| Element::NumberWithExpression { .. }
			| Element::VariableOrFunction(_)
			| Element::Number(_) => {},
		}
	}

	fn try_convert_to_variable(&mut self) -> bool {
		if let Element::VariableOrFunction(name) = self {
			*self = Element::Variable(name.to_owned());
			true
		} else {
			false
		}
	}

	pub(crate) fn anything_unparsed(&self) -> bool {
		match self {
			Element::Brackets(_) | Element::String(_) => true,
			Element::Plus(elements)
			| Element::Multiply(elements)
			| Element::Function { arguments: elements, .. }
			| Element::FunctionWithExpression { arguments: elements, .. } => {
				elements.iter().any(Element::anything_unparsed)
			},
			Element::Pow(base, exponent) => base.anything_unparsed() || exponent.anything_unparsed(),
			Element::Negate(element) => element.anything_unparsed(),
			Element::Variable(_)
			| Element::Number(_)
			| Element::VariableOrFunction(_)
			| Element::NumberWithExpression { .. } => false,
		}
	}
}

fn list_contains_char(input: &[Element], delimiter: char) -> bool {
	input.iter().any(|e| if let Element::String(s) = e { s.contains(delimiter) } else { false })
}

fn split_list_by_char(input: &[Element], delimiter: char) -> Option<Vec<Element>> {
	if !list_contains_char(input, delimiter) {
		return None;
	}
	let mut groups = Vec::new();
	let mut current_group = Vec::new();

	fn add_current_group(groups: &mut Vec<Element>, current_group: &mut Vec<Element>) {
		if !current_group.is_empty() {
			let mut group_to_add = mem::take(current_group);
			if group_to_add.len() == 1 {
				groups.push(group_to_add.pop().unwrap());
			} else {
				groups.push(Element::Brackets(group_to_add));
			}
		}
	}
	for element in input {
		if let Element::String(str) = element {
			if !str.contains(delimiter) {
				current_group.push(element.clone());
			} else {
				let parts: Vec<&str> = str.split(delimiter).collect();
				for (i, part) in parts.iter().enumerate() {
					if i == 0 {
						if part.is_empty() {
							add_current_group(&mut groups, &mut current_group);
						} else {
							current_group.push(Element::String(part.to_string()));
						}
					} else if i > 0 {
						add_current_group(&mut groups, &mut current_group);
						if !part.is_empty() {
							current_group.push(Element::String(part.to_string()));
						}
					}
				}
			}
		} else {
			current_group.push(element.clone());
		}
	}

	add_current_group(&mut groups, &mut current_group);

	Some(groups)
}

fn split_string_by_char(input: &str, delimiter: char) -> Option<Vec<Element>> {
	if !input.contains(delimiter) {
		return None;
	}
	input
		.split(delimiter)
		.filter(|s| !s.is_empty())
		.map(|s| Element::String(s.to_string()))
		.collect::<Vec<_>>()
		.into()
}
