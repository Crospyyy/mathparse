use crate::benchmarking::Benchmark;
use crate::calculation::create_context;
use crate::calculation::expression_values::FunctionExpression;
use crate::outer_store_interation::RunPrecision;
use crate::storing::FormulaStore;
use crate::{Element, Number, RoundingMode, only_in_debug};
use anyhow::Result;
use astro_float::BigFloat;
use astro_float::ctx::Context;
use num_rational::BigRational;
use num_traits::ToPrimitive;
use std::collections::HashSet;
use std::fmt::{Debug, Display, Formatter};
use std::ops::RangeInclusive;
use thiserror::Error;

#[derive(Debug, PartialEq)]
pub enum DynamicResult {
	Exact(BigRational),
	Checked { num: BigFloat, precision: usize },
	ReachedLimit(BigFloat),
}

impl DynamicResult {
	pub fn as_i32(&self) -> Option<i32> {
		match self {
			DynamicResult::Exact(r) => {
				if r.is_integer() {
					r.to_i32()
				} else {
					None
				}
			},
			_ => None,
		}
	}

	pub fn is_exact(&self) -> bool {
		matches!(self, DynamicResult::Exact(_))
	}
}

#[derive(Error, Debug)]
pub enum FormulaEvaluationError {
	#[error("Contains unparsed elements")]
	UnparsedElements,
	#[error("Contains unexpanded variables or functions")]
	UnexpandedElements,
	#[error("Contains function call with invalid argument count")]
	FunctionCallWithInvalidArgumentCount,
}

impl Element {
	fn eval(&self, ctx: &mut Context) -> Result<Number, FormulaEvaluationError> {
		match self {
			Element::Brackets(_) | Element::String(_) => Err(FormulaEvaluationError::UnparsedElements),
			Element::Variable(_) | Element::Function { .. } | Element::VariableOrFunction(_) => {
				Err(FormulaEvaluationError::UnexpandedElements)
			},
			Element::Plus(elements) => {
				let values: Vec<Number> = elements.iter().map(|e| e.eval(ctx)).collect::<Result<_, _>>()?;
				if let Some(nan) = values.iter().find(|v| v.is_nan()) {
					return Ok(nan.clone());
				}
				let product = values.iter().fold(Number::from(0), |acc, n| acc.plus(n, ctx));
				Ok(product)
			},
			Element::Multiply(elements) => {
				let values: Vec<_> = elements.iter().map(|e| e.eval(ctx)).collect::<Result<_, _>>()?;
				if let Some(nan) = values.iter().find(|v| v.is_nan()) {
					return Ok(nan.clone());
				}
				if values.iter().any(|v| *v == 0) {
					return Ok(Number::from(0));
				}
				let product = values.iter().fold(Number::from(1), |acc, n| acc.mul(n, ctx));
				Ok(product)
			},
			Element::Negate(e) => e.eval(ctx).map(|n| n.neg()),
			Element::Number(n) => Ok(n.clone()),
			Element::Pow(b, e) => Ok(b.eval(ctx)?.pow(&e.eval(ctx)?, ctx)),
			Element::NumberWithExpression { expr_value } => Ok(expr_value.get_function()(ctx)),
			Element::FunctionWithExpression { arguments, expr_value } => {
				let count = expr_value.get_param_count();
				if !count.number_would_be_valid(arguments.len()) {
					return Err(FormulaEvaluationError::FunctionCallWithInvalidArgumentCount);
				}
				match expr_value.get_function() {
					FunctionExpression::SingleArgument(fun) => Ok(fun(&arguments[0].eval(ctx)?, ctx)),
					FunctionExpression::MultipleArguments(fun) => {
						let args = arguments.iter().map(|a| a.eval(ctx)).collect::<Result<Vec<_>, _>>()?;
						Ok(fun(args, ctx))
					},
				}
			},
		}
	}

	pub fn eval_dynamic_precision(
		&self, min_precision: usize, max_precision: usize,
	) -> Result<DynamicResult> {
		let mut precision = min_precision;
		let mut ctx;
		let mut last_rounded = None;

		while last_rounded.is_none() || precision <= max_precision {
			ctx = create_context(precision);
			let result = self.eval(&mut ctx)?;

			let mut rounded = match result {
				Number::Float(f) => f,
				Number::Rational(r) => {
					return Ok(DynamicResult::Exact(r));
				},
			};
			if rounded.is_nan() {
				return Ok(DynamicResult::Checked { num: rounded, precision });
			}
			rounded = rounded.round(min_precision, RoundingMode::ToEven);
			rounded.set_inexact(true);

			if let Some(last_rounded) = &mut last_rounded
				&& *last_rounded == rounded
			{
				return Ok(DynamicResult::Checked { num: rounded, precision });
			}

			last_rounded = Some(rounded);
			precision *= 2;
		}
		Ok(DynamicResult::ReachedLimit(last_rounded.unwrap()))
	}

	fn get_all_unexpanded_names(&self, names: &mut HashSet<String>) {
		match self {
			Element::Brackets(_)
			| Element::String(_)
			| Element::Number(_)
			| Element::NumberWithExpression { .. }
			| Element::FunctionWithExpression { .. } => {},
			Element::Plus(e) | Element::Multiply(e) => {
				e.iter().for_each(|el| el.get_all_unexpanded_names(names));
			},
			Element::Pow(a, b) => {
				a.get_all_unexpanded_names(names);
				b.get_all_unexpanded_names(names);
			},
			Element::Negate(x) => x.get_all_unexpanded_names(names),
			Element::Function { name, arguments } => {
				names.insert(name.to_owned());
				arguments.iter().for_each(|arg| arg.get_all_unexpanded_names(names));
			},
			Element::Variable(name) | Element::VariableOrFunction(name) => {
				names.insert(name.to_owned());
			},
		}
	}
}

#[derive(Error, Debug)]
pub enum EvaluationError {
	#[error("Could not parse formula: {0}")]
	CouldNotParse(String),
	// todo find a better way to to error handling
}

#[derive(Debug, Error)]
struct ExpansionError;
impl Display for ExpansionError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "Could not expand formula")
	}
}

impl FormulaStore {
	pub(crate) fn expand_and_optimize(
		&self, formula: &mut Element, benchmark: &mut Benchmark, exclude_from_expansion: &HashSet<String>,
	) -> Result<()> {
		benchmark.benchmark("Formula Optimization", || formula.optimize_and_reduce());
		benchmark.benchmark("Expansion", || self.expand_formula(formula, exclude_from_expansion))?;
		benchmark.benchmark("Formula Optimization", || formula.optimize_and_reduce());
		Ok(())
	}

	pub(crate) fn expand_formula(&self, formula: &mut Element, ignore_names: &HashSet<String>) -> Result<()> {
		// todo look into how to do this more efficiently
		let mut all_names = HashSet::new();
		loop {
			let before = all_names.clone();
			all_names.clear();
			formula.get_all_unexpanded_names(&mut all_names);
			all_names = all_names.difference(ignore_names).cloned().collect();
			//only_in_debug!(dbg!(&all_names));

			if all_names.is_empty() {
				break;
			}
			if all_names == before {
				return Err(ExpansionError.into());
			}
			for name in all_names.iter() {
				formula.insert_symbol(&self.get_insertion_element_expanded(name, ignore_names)?)?;
			}
		}

		Ok(())
	}

	pub fn eval_new(
		&self, formula_str: &str, precision: RunPrecision, benchmark: &mut Benchmark,
	) -> Result<DynamicResult> {
		let mut formula = benchmark
			.benchmark("Parsing", || Element::parse(formula_str).map_err(EvaluationError::CouldNotParse))?;
		
		benchmark.bench_with_inner("Expansion and Optimization", |b| {
			self.expand_and_optimize(&mut formula, b, &HashSet::new())
		})?;

		// todo add result caching

		match precision {
			RunPrecision::Fixed(p) => {
				let mut ctx = create_context(p);
				match benchmark.benchmark("Evaluation", || formula.eval(&mut ctx))? {
					Number::Rational(r) => Ok(DynamicResult::Exact(r)),
					Number::Float(f) => Ok(DynamicResult::Checked { num: f, precision: p }),
				}
			},
			RunPrecision::Dynamic(min, max) => benchmark
				.benchmark("Dynamic Precision Evaluation", || formula.eval_dynamic_precision(min, max)),
		}
	}

	pub const DEFAULT_PRECISION_RANGE: RangeInclusive<u32> = 512..=(1 << 20);
}

#[cfg(test)]
mod tests {
	use crate::benchmarking::Benchmark;
	use crate::calculation::create_default_context;
	use crate::outer_store_interation::RunOptions;
	use crate::outer_store_interation::RunPrecision;
	use crate::storing::FormulaStore;
	use crate::{DynamicResult, Element, FormattingOptions, Number};
	use astro_float::ctx::Context;
	use astro_float::{BigFloat, Error};
	use num_rational::BigRational;
	use std::str::FromStr;

	#[test]
	fn test_as_i32() {
		fn create_exact_result(num: i64) -> DynamicResult {
			DynamicResult::Exact(BigRational::from_integer(num.into()))
		}
		fn create_exact_result_rat(num: i64, denom: i64) -> DynamicResult {
			DynamicResult::Exact(BigRational::new(num.into(), denom.into()))
		}

		fn create_checked_result(s: &str) -> DynamicResult {
			DynamicResult::Checked { num: BigFloat::from_str(s).unwrap(), precision: 64 }
		}

		fn create_reached_limit_result(s: &str) -> DynamicResult {
			DynamicResult::ReachedLimit(BigFloat::from_str(s).unwrap())
		}

		assert_eq!(create_exact_result(42).as_i32(), Some(42));
		assert_eq!(create_checked_result("42").as_i32(), None);
		assert_eq!(create_reached_limit_result("42").as_i32(), None);

		assert_eq!(create_exact_result(-1).as_i32(), Some(-1));
		assert_eq!(create_checked_result("-1").as_i32(), None);
		assert_eq!(create_reached_limit_result("-1").as_i32(), None);

		assert_eq!(create_exact_result_rat(3, 2).as_i32(), None);
		assert_eq!(create_reached_limit_result("2.5").as_i32(), None);

		assert_eq!(create_exact_result(4294967296).as_i32(), None);
		assert_eq!(create_exact_result(4294967295).as_i32(), None);
		assert_eq!(create_exact_result(2147483648).as_i32(), None);
		assert_eq!(create_exact_result(2147483647).as_i32(), Some(2147483647));
	}

	#[test]
	fn test_formula_evaluation() {
		let mut ctx = create_default_context();
		let inputs = [
			("0/0", Some(Number::nan(Error::DivisionByZero.into()))),
			("1/0", Some(Number::nan(Error::DivisionByZero.into()))),
			("1+2", Some(3.into())),
			("1+2*3", Some(7.into())),
			("1+2*3-4/2", Some(5.into())),
			("(1+2)*3", Some(9.into())),
			("(1+2)*(3-4)", Some((-3).into())),
			("x/x-x", None),
			// Potenzierungen
			("2^3", Some(8.into())),
			("2^3^2", Some(512.into())), // 2^(3^2) = 2^9
			("(2^3)^2", Some(64.into())),
			("-2^2", Some((-4).into())), // -(2^2)
			("(-2)^2", Some(4.into())),
			("16^(1/2)", Some(4.into())),
			// Negation
			("-1", Some((-1).into())),
			("--1", Some(1.into())),
			("-(1+2)", Some((-3).into())),
			// Komplexere Ausdrücke
			("1+2*3+4", Some(11.into())),
			("(1+2)*(3+4)", Some(21.into())),
			("2*3+4*5", Some(26.into())),
			("2*(3+4)*5", Some(70.into())),
			// Division
			("8/2", Some(4.into())),
			("8/2/2", Some(2.into())),
			("8/(2*2)", Some(2.into())),
			(
				"1/2+1/3",
				Some(
					Number::from(1)
						.div(&Number::from(2), &mut ctx)
						.plus(&Number::from(1).div(&Number::from(3), &mut ctx), &mut ctx),
				),
			),
			// Nullwerte
			("0+0", Some(0.into())),
			("0*5", Some(0.into())),
			("5*0", Some(0.into())),
			("0^2", Some(0.into())),
			// Dezimalzahlen
			("1.5+2.5", Some(4.into())),
			("3.14*2", Some(Number::from_string("3.14").unwrap().mul(&Number::from(2), &mut ctx))),
			(
				"10.5/2.5",
				Some(
					Number::from_string("10.5").unwrap().div(&Number::from_string("2.5").unwrap(), &mut ctx),
				),
			),
			// Fehlerfälle
			("", None),
		];
		println!("Starting formula evaluation tests");
		inputs.into_iter().for_each(|(i, o)| {
			println!("Testing formula evaluation for input: {}", i);
			let mut ctx1 = Context::new(
				1024,
				astro_float::RoundingMode::ToEven,
				astro_float::Consts::new().unwrap(),
				-100000,
				100000,
			);
			let output = Element::parse(i).ok().as_ref().and_then(|e| e.eval(&mut ctx1).ok());
			assert_eq!(output, o);
		});
	}

	#[test]
	fn test_eval_formula_store() {
		let mut store = FormulaStore::new_empty();
		store.add_symbol_from_string("f(x)=x^2", false).unwrap();
		store.add_symbol_from_string("a=4", false).unwrap();
		assert_eq!(
			store
				.eval_new("f(a)", RunPrecision::default(), &mut Benchmark::new("eval"))
				.unwrap()
				.to_string_detailed(FormattingOptions::default())
				.get_string(),
			"16"
		);
		assert!(store.eval_new("f", RunPrecision::default(), &mut Benchmark::new("eval")).is_err());
	}

	#[test]
	fn test_expression_functions() {
		let store = &mut FormulaStore::new_with_default_symbols();

		// test signatures
		fn generate_fn_call(fun_name: &str, arg_count: usize) -> String {
			format!("{}({})", fun_name, vec!["0"; arg_count].join(","))
		}
		let test_with_multiple_arg_counts = |function_names: &[&str], expected: fn(usize) -> bool| {
			for function in function_names {
				for i in 0..5 {
					println!("Testing function: {} with {} arguments", function, i);
					assert_eq!(
						store
							.eval_new(
								&generate_fn_call(function, i),
								RunPrecision::default(),
								&mut Benchmark::new("Evaluate")
							)
							.map(|r| r.as_i32())
							.is_ok(),
						expected(i)
					);
				}
			}
		};

		let functions_that_take_one_argument =
			["sin", "cos", "tan", "sqrt", "abs", "log2", "log10", "ln", "floor", "ceil", "round"];
		let functions_that_take_zero_or_more_arguments = ["sum"];
		let functions_that_take_one_or_more_arguments = ["avg", "max", "min", "median"];

		test_with_multiple_arg_counts(&functions_that_take_one_argument, |i| i == 1);
		test_with_multiple_arg_counts(&functions_that_take_zero_or_more_arguments, |_| true);
		test_with_multiple_arg_counts(&functions_that_take_one_or_more_arguments, |i| i >= 1);

		store.quick_eval("rem(-2, 2)", 0);
		store.quick_eval("rem(-1, 2)", 1);
		store.quick_eval("rem(0, 2)", 0);
		store.quick_eval("rem(1, 2)", 1);
		store.quick_eval("rem(2, 2)", 0);
		store.quick_eval("abs(-123)", 123);
		store.quick_eval("avg(1,2,3)", 2);
		store.quick_eval("max(1,2,3)", 3);
		store.quick_eval("min(1,2,3)", 1);
		store.quick_eval("sum(1,2,3)", 6);
		store.quick_eval("median(1,2,3)", 2);
		store.quick_eval2("median(1,2,3,4)", "2.5");
		store.quick_eval2("median(2,3,4,1)", "2.5");
		store.quick_eval2("median(3,4,1,2)", "2.5");
		store.quick_eval2("median(4,1,2,3)", "2.5");
		store.quick_eval2("median(4,1,2,3)", "2.5");
		store.quick_eval("min(1,2,3)", 1);
		store.quick_eval("sum()", 0);
		store.quick_eval("sum(0)", 0);
		store.quick_eval("sum(0,0)", 0);
		store.quick_eval("ceil(123.456)", 124);
		store.quick_eval("floor(123.456)", 123);
		store.quick_eval("floor(-123.456)", -124);

		store.quick_eval("ceil(123.456)", 124);
		store.quick_eval("ceil(-123.456)", -123);

		store.quick_eval("round(123.456)", 123);
		store.quick_eval("round(123.789)", 124);
		store.quick_eval("round(-123.456)", -123);
		store.quick_eval("round(-123.789)", -124);

		store.add_symbol_from_string("f(x)=sin(x)", false).unwrap();
		assert!(store.add_symbol_from_string("g(x)=undefined(x)", false).is_err());
		store.add_symbol_from_string("g(x)=f(x)+cos(x)", false).unwrap();

		store.quick_eval("f(0)", 0);
		store.quick_eval("g(0)", 1);

		store.add_symbol_from_string("good_sum(x,y) = sum(x,y) + sum(x,y)", false).unwrap();
		store.quick_eval("good_sum(1,2)", 1 + 2 + 1 + 2);
		store.add_symbol_from_string("weird_sum(x,y)=sum(x,y)+sum(x,y,1)", false).unwrap();
		store.quick_eval("weird_sum(1,2)", 1 + 2 + 1 + 2 + 1);
	}

	#[test]
	fn test_general_evaluation() {
		let mut store = FormulaStore::new_empty();
		store.define_default_symbols().unwrap();

		fn count_digits(s: &str) -> usize {
			let s: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
			s.trim_start_matches("0").len()
		}
		macro_rules! check_calculation {
			($formula:expr, $expected:expr) => {
				check_calculation!($formula, $expected, count_digits($expected))
			};
			($formula:expr, $expected:expr, $rounding:expr) => {{
				let (formula, output) = ($formula, $expected);
				let fmt = FormattingOptions::default().with_rounding($rounding);
				let result = store
					.run_new(formula, RunOptions::default(), &mut Benchmark::new("Evaluate"))
					.calculation_result()
					.unwrap()
					.to_string_detailed(fmt)
					.get_string()
					.clone();
				assert_eq!(
					result, output,
					"For formula '{}', expected '{}', but got '{}'",
					formula, output, result
				);
			}};
		}

		check_calculation!(
			"((12 + 3) * sin(0.5)) / (2 ^ 3)",
			"0.89892288488288062551241487852919635265338131488862626597865614961037813"
		);
		check_calculation!(
			"(cos(1) + (7 - (3 + 2))) * ( (10 / 2) + tan(0.25) )",
			"13.350157200603297892624056197885460276733113344409248718867431711422992"
		);
		check_calculation!(
			"( ( (25 / 5) + (2 ^ 3) ) * ( (sqrt(9) - 1) + sin(3.14 / 2) ) )",
			"38.99999587811385"
		);
		check_calculation!("(((3.5 + 1.5) * (2.2 - 0.2)) / ( (10 - 8) ^ 2 )) + cos(0)", "3.5", 100);
		check_calculation!(
			"( (100 - (50 / (5 + 5))) * ( (20 / (2 ^ 2)) + 3 ) ) - tan(1)",
			"758.4425922753451"
		);
		check_calculation!(
			"( ( ( (2 + 3) * (4 + 1) ) - ( (9 - 7) * (8 / 2) ) ) ^ 2 ) + sin(2)",
			"289.9092974268257"
		);
		check_calculation!(
			"( (ln(e ^ 2)) + (ln(1000) - ln(10)) + cos(2)) / ( (5 + 5) / 2 )",
			"1.23780466988819"
		);
		check_calculation!("((sin(1) ^ 2) + (cos(1) ^ 2)) * 42", "42", 100);
		check_calculation!("(tan(0.5) + (sin(0.25) * cos(0.75))) / (2 ^ (1/2))", "0.5142965902018098");
		check_calculation!(
			"((ln(100) + ln(e)) * (sin(3.14159) + cos(3.14159 / 2)))",
			"0.00002231073359233416412239818251719053988230415986601072682456917655910148"
		);

		check_calculation!("1/0", "NaN");
		check_calculation!("0/0", "NaN");
		check_calculation!("sqrt(-1)", "NaN");
		check_calculation!("ln(0)", "-Inf");
		check_calculation!("ln(-10)", "NaN");
		check_calculation!("0^0", "1");
		check_calculation!("(1/0) - (1/0)", "NaN");
		check_calculation!("0.1 + 0.2", "0.3");
		check_calculation!("(-2)^0.5", "NaN");
		check_calculation!("5 * - -2", "10");

		check_calculation!(
			"sin(123)",
			"-0.459903490689591251292435715293231810808580607381042580927742868029959"
		);
		check_calculation!(
			"cos(123)",
			"-0.887968906691855428978322569442621150811865560981614293926181503346636"
		);
		check_calculation!(
			"tan(123)",
			"0.5179274715856551831319240756392856882012784222828477932177680554912204"
		);
		check_calculation!(
			"sqrt(123)",
			"11.090536506409417162051600102609932918463376742454020022877312839085002"
		);
		check_calculation!("sqrt(15129)", "123");
		check_calculation!(
			"log2(123)",
			"6.9425145053392398746197102514707252094534827932226660599060107680885122"
		);
		check_calculation!("log2(1024)", "10");
	}
}
