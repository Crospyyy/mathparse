use crate::calculation::create_default_context;
use crate::formula_short::*;
use crate::parsing::implementation::ParseError;
use crate::{Element, ElementParsed};
use astro_float::ctx::Context;

#[test]
fn test_parsing_on_manual_formulas() {
	let inputs = [
		(
			"((x/x-x)*-x^x)/(x-x)^-x",
			Some(mul([
				mul([plus([mul([var("x"), inv(var("x"))]), neg(var("x"))]), neg(pow(var("x"), var("x")))]),
				inv(pow(plus([var("x"), neg(var("x"))]), neg(var("x")))),
			])),
		),
		(
			"(x/x+-x)*x^x",
			Some(mul([plus([mul([var("x"), inv(var("x"))]), neg(var("x"))]), pow(var("x"), var("x"))])),
		),
		("x/x/x/x", Some(mul([var("x"), inv(var("x")), inv(var("x")), inv(var("x"))]))),
		("x/x-x", Some(plus([mul([var("x"), inv(var("x"))]), neg(var("x"))]))),
		("123", Some(num("123"))),
		("123-1", Some(plus([num("123"), neg(num("1"))]))),
		("123--1", Some(plus([num("123"), neg(neg(num("1")))]))),
		("x", Some(var_or_fun("x"))),
		("1+((2))", Some(plus([num("1"), num("2")]))),
		("a,b,c", None),
		("a(a,c)", Some(fun("a", [var_or_fun("a"), var_or_fun("c")]))),
		("a(a+c)", Some(fun("a", [plus([var("a"), var("c")])]))),
		("m+a(a,b+c)", Some(plus([var("m"), fun("a", [var_or_fun("a"), plus([var("b"), var("c")])])]))),
		("fun3(some_fun)", Some(fun("fun3", [var_or_fun("some_fun")]))),
		("fun(12, fun(1, 2))", Some(fun("fun", [num("12"), fun("fun", [num("1"), num("2")])]))),
		("fun()", Some(fun("fun", []))),
		("fun()-fun()", Some(plus([fun("fun", []), neg(fun("fun", []))]))),
		("1+2*3-4/2", Some(plus([num("1"), mul([num("2"), num("3")]), neg(mul([num("4"), inv(num("2"))]))]))),
		("var^--var2", Some(pow(var("var"), neg(neg(var("var2")))))),
		("11-3-27/60", Some(plus([num(11), neg(num(3)), neg(mul([num(27), inv(num(60))]))]))),
	];
	println!("Starting formula parsing tests");
	inputs.into_iter().for_each(|(i, o)| debug_formula_parsing_process(i, o));
}

fn debug_formula_parsing_process(input: &str, expected_output: Option<ElementParsed>) {
	let mut ctx = create_default_context();
	let expected_output = expected_output;
	let cow = Element::preprocess_string(input);

	println!();
	print_heading("Starting formula parsing");
	println!();

	println!("Input: {}", cow);
	let chars = cow.chars().collect::<Vec<_>>();

	print_heading("0. Bracketize and process operations");

	let mut start = 0;
	let mut brackets = Element::resolve_brackets(&chars, &mut start);

	println!("{}", brackets.get_string(&mut ctx));
	println!("{}", brackets.get_debug_string());
	let mut result = Ok(());
	'processing: {
		debug_print_step("0,5. Resolve functions", &mut brackets, Element::resolve_functions, &mut ctx);
		debug_print_step("1. Processing '+'", &mut brackets, Element::process_plus, &mut ctx);
		debug_print_step("2. Processing '-'", &mut brackets, Element::process_minus, &mut ctx);
		debug_print_step("3. Processing '*'", &mut brackets, Element::process_multiply, &mut ctx);
		debug_print_step("4. Processing '/'", &mut brackets, Element::process_divide, &mut ctx);
		debug_print_step("5. Processing '-' again", &mut brackets, Element::process_minus, &mut ctx);
		debug_print_step(
			"6. Processing '^'",
			&mut brackets,
			|e| {
				let output = e.process_pow();
				if output.is_err() {
					result = output;
				}
			},
			&mut ctx,
		);
		if result.is_err() {
			break 'processing;
		}
		debug_print_step("7. Processing '-' again", &mut brackets, Element::process_minus, &mut ctx);
		debug_print_step(
			"8. Convert to numbers and variables",
			&mut brackets,
			Element::process_numbers_and_variables,
			&mut ctx,
		);
		debug_print_step(
			"9. Removing unneeded outer brackets",
			&mut brackets,
			Element::remove_unneeded_outer_brackets,
			&mut ctx,
		);
		debug_print_step(
			"10. Convert ambiguous symbols to variables where possible",
			&mut brackets,
			Element::convert_to_variables_where_possible,
			&mut ctx,
		);
	}

	let output: Result<ElementParsed, ParseError> = if let Err(msg) = result {
		Err(ParseError::ProcessingPow(msg))
	} else {
		brackets.try_into().map_err(ParseError::UnparsedElementLeft)
	};
	let manual_output = output.ok();
	assert_eq!(manual_output, Element::parse(input).ok());
	assert_eq!(manual_output, expected_output);
}

fn debug_print_step(
	step: &str, element: &mut Element, operation: impl FnOnce(&mut Element), ctx: &mut Context,
) {
	print_heading(step);
	operation(element);
	println!("{}", element.get_string(ctx));
	println!("{}", element.get_debug_string());
}

fn print_heading(step: &str) {
	println!("##### {}", step);
}

mod formula_generation {
	use crate::parsing::testing::debug_formula_parsing_process;
	use crate::printing::Formula;
	use crate::{Element, ElementParsed};
	use rand::random_range;

	impl Formula {
		fn random_number() -> f64 {
			random_range(0.0..=100.0)
		}

		fn random_var_name() -> String {
			format!("var{}", random_range(1..=10))
		}

		fn generate_random(depth: usize) -> Self {
			match random_range(0..if depth == 0 { 2 } else { 8 }) {
				0 => Formula::Number(Self::random_number().to_string()),
				1 => Formula::Variable(Self::random_var_name()),
				2 => Formula::Plus(
					(0..random_range(2..=4)).map(|_| Self::generate_random(depth - 1)).collect(),
				),
				3 => Formula::Multiply(
					(0..random_range(2..=4)).map(|_| Self::generate_random(depth - 1)).collect(),
				),
				4 => Formula::Negate(Box::new(Self::generate_random(depth - 1))),
				5 => Formula::Pow(
					Box::new(Self::generate_random(depth - 1)),
					Box::new(Self::generate_random(depth - 1)),
				),
				6 => Formula::Division(
					Box::new(Self::generate_random(depth - 1)),
					Box::new(Self::generate_random(depth - 1)),
				),
				7 => {
					let name = format!("f{}", random_range(1..=10));
					let args = (0..random_range(1..=3)).map(|_| Self::generate_random(depth - 1)).collect();
					Formula::Function { name, arguments: args }
				},
				_ => unreachable!(),
			}
		}
	}

	#[test]
	fn test_parsing_randomly_generated_formulas() {
		println!("Starting formula generation tests");
		for _ in 0..100 {
			let formula = Formula::generate_random(3);
			let parsed_result = Element::parse(&formula.to_string());
			if parsed_result.is_err() {
				println!("Failed to parse: {}", formula);
				debug_formula_parsing_process(
					&formula.to_string(),
					Some(ElementParsed::Variable("Something".to_owned())),
				)
			}
		}
	}
}
