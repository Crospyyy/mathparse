use crate::expression_values::{ExpressionFunType, ExpressionNumType};
use crate::formula_short::{fun_expr, inv, mul, neg, num, num_expr, pow};
use crate::{Element, FormulaStore, Number, formula};
use astro_float::Error;
use macros::formula_matches;
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};
use std::cmp::PartialEq;
use std::ops::{Add, Mul, Neg, Rem};
use strum::{EnumCount, IntoEnumIterator};

#[macro_export]
macro_rules! quick_match {
	($input:expr,$pat:pat => $expr:expr) => {
		match $input {
			$pat => Some($expr),
			_ => None,
		}
	};
}

fn remove_inverse_elements(elements: &mut Vec<Element>, inverse_check: impl Fn(&Element, &Element) -> bool) {
	if elements.len() >= 2 {
		let mut to_remove = vec![false; elements.len()];
		'outer: for i in 0..elements.len() - 1 {
			for j in i + 1..elements.len() {
				if to_remove[j] {
					continue;
				}
				if inverse_check(&elements[i], &elements[j]) {
					to_remove[i] = true;
					to_remove[j] = true;
					continue 'outer;
				}
			}
		}
		for (i, _) in to_remove.into_iter().enumerate().filter(|x| x.1).rev() {
			elements.remove(i);
		}
	}
}

impl Element {
	fn run_on_children(&mut self, operation: &mut impl Fn(&mut Element) -> bool) -> bool {
		match self {
			Element::Plus(elements)
			| Element::Multiply(elements)
			| Element::Function { arguments: elements, .. }
			| Element::FunctionWithExpression { arguments: elements, .. } => {
				elements.iter_mut().map(operation).reduce(|a, b| a || b).unwrap_or(false)
			},
			Element::Pow(base, exponent) => operation(base) || operation(exponent),
			Element::Negate(element) => operation(element),
			Element::Variable(_)
			| Element::Brackets(_)
			| Element::String(_)
			| Element::VariableOrFunction(_)
			| Element::Number(_)
			| Element::NumberWithExpression { .. } => false,
		}
	}

	fn handle_empty_or_one_element(elements: &[Element], neutral_element: u16) -> Option<Element> {
		if elements.is_empty() {
			Some(formula!(num(neutral_element as i32)))
		} else if elements.len() == 1 {
			Some(elements[0].clone())
		} else {
			None
		}
	}

	pub fn optimize_and_reduce(&mut self) {
		match self {
			// Elements, which can't be optimized further
			Element::Brackets(_)
			| Element::String(_)
			| Element::Variable(_)
			| Element::VariableOrFunction(_)
			| Element::NumberWithExpression { .. }
			| Element::Number(_) => {},

			// Elements, which can be optimized
			Element::Function { arguments, .. } => {
				for a in arguments {
					a.optimize_and_reduce();
				}
			},
			Element::FunctionWithExpression { arguments, expr_value } => {
				for a in arguments.iter_mut() {
					a.optimize_and_reduce();
				}
				match expr_value {
					ExpressionFunType::Sin => {
						let [arg] = arguments.as_slice() else {
							return;
						};
						let divided = mul([arg.clone(), inv(num_expr(ExpressionNumType::Pi))]);
						let rem = fun_expr(ExpressionFunType::Rem, [divided.clone(), num(2)]);
						let mut mul = mul([rem.clone(), num(2)]);
						mul.optimize_and_reduce();
						if let Some(x) = formula_matches!(mul, num(x)) {
							if x == 0 || x.abs() == 2 {
								*self = Number::from(0).into();
								return;
							}
							if x.abs() == 1 {
								*self = Number::from(if x == 1 { 1 } else { -1 }).into();
								return;
							}
						}
						let corrected = fun_expr(ExpressionFunType::SinWithRadians, [mul]);
						*self = corrected;
					},
					ExpressionFunType::Rem => {
						let [arg1, arg2] = arguments.as_slice() else {
							return;
						};
						let Some(arg1_num) =
							formula_matches!(arg1, num(x)).and_then(|x| x.get_exact_rational())
						else {
							return;
						};
						let Some(arg2_num) =
							formula_matches!(arg2, num(x)).and_then(|x| x.get_exact_rational())
						else {
							return;
						};
						let mut result = arg1_num.clone().rem(&arg2_num);
						if result.is_negative() {
							result += &arg2_num;
						}
						*self = Number::from(result).into();
					},
					_ => {},
				}
			},
			Element::Plus(elements) => {
				let inverse_check = |a: &Element, b: &Element| {
					formula_matches!(a, neg({ b })) || formula_matches!(b, neg({ a }))
				};

				let result = list_element_optimization(
					elements,
					BigRational::add,
					inverse_check,
					BigRational::is_zero,
					false,
					true,
				);
				if let Some(replacement) = Self::handle_empty_or_one_element(elements, 0) {
					*self = replacement
				}
			},
			Element::Multiply(elements) => {
				let inverse_check = |a: &Element, b: &Element| {
					formula_matches!(a, pow({ b }, num(-1))) || formula_matches!(b, pow({ a }, num(-1)))
				};

				let result = list_element_optimization(
					elements,
					BigRational::mul,
					inverse_check,
					BigRational::is_one,
					true,
					false,
				);
				if let Some(replacement) = Self::handle_empty_or_one_element(elements, 1) {
					*self = replacement
				}
				if result.add_outer_neg {
					*self = neg(self.clone());
					self.optimize_and_reduce();
				}
			},
			Element::Pow(this_base, this_exponent) => {
				this_base.optimize_and_reduce();
				this_exponent.optimize_and_reduce();

				fn is_even(x: &BigRational) -> bool {
					let rem = BigRational::from(BigInt::from(2));
					x.rem(&rem).is_zero()
				}

				// optimize the element itself
				if let Some(this_base_number) = formula_matches!(this_base.as_ref(), num(x)) {
					if this_base_number.get_exact_rational().is_some_and(|n| n.is_one()) {
						// 1^x = 1
						*self = Number::from(1).into();
						return;
					}
					if this_base_number.is_zero() {
						if let Some(this_exponent_num) = formula_matches!(this_exponent.as_ref(), num(x)) {
							if this_exponent_num.is_positive() {
								// zero^positive_number = 0
								*self = Number::from(0).into();
								return;
							}
							if this_exponent_num.is_negative() {
								// zero^negative_number = NaN (division by zero)
								*self = Number::nan(Some(Error::DivisionByZero)).into();
								return;
							}
							if this_exponent_num.is_zero() {
								// zero^0 = 1 (by convention)
								*self = Number::from(1).into();
								return;
							}
						}
					} else if this_base_number.is_negative()
						&& formula_matches!(this_exponent.as_ref(), num(x))
							.and_then(|x| x.get_exact_rational())
							.is_some_and(|x| x.is_integer() && is_even(&x))
					{
						// (-a)^(even integer) = (a)^(even integer)
						*this_base = Box::new(this_base_number.abs().into());
						return;
					}
					if formula_matches!(this_exponent.as_ref(), num(-1))
						&& let Some(r) = this_base_number.get_exact_rational()
					{
						*self = Number::from(r.recip()).into();
						return;
					}
				}

				if let Some(this_exponent_number) = formula_matches!(this_exponent.as_ref(), num(x)) {
					if this_exponent_number.get_exact_rational().is_some_and(|n| n.is_one()) {
						// x^1 = x
						*self = this_base.as_ref().clone();
						return;
					}
					if this_exponent_number.is_zero() {
						// x^0 = 1
						*self = Number::from(1).into();
						return;
					}
				}

				// optimize nested powers
				if let Some((inner_b, inner_p)) = formula_matches!(this_base.as_ref(), pow(x, x)) {
					let base_is_positive_num =
						formula_matches!(inner_b, num(x)).is_some_and(Number::is_positive);
					let inner_exp_rat_int = formula_matches!(inner_p, num(x))
						.and_then(Number::get_exact_rational)
						.as_ref()
						.is_some_and(BigRational::is_integer);
					let outer_exp_rat_int = formula_matches!(this_exponent.as_ref(), num(x))
						.and_then(Number::get_exact_rational)
						.as_ref()
						.is_some_and(BigRational::is_integer);
					let exponent_is_even_and_outer_exp_is_int = inner_exp_rat_int && outer_exp_rat_int;
					if base_is_positive_num || exponent_is_even_and_outer_exp_is_int {
						// if the base is positive, we can just multiply the exponents
						let new_pow = mul([inner_p.clone(), this_exponent.as_ref().clone()]);
						*self = formula!(pow(inner_b.clone(), new_pow));
						self.optimize_and_reduce();
					}
				} else if let Some(inner) = formula_matches!(this_base.as_ref(), mul(x..)) {
					let new_elements = inner
						.iter()
						.map(|el| pow((*el).clone(), this_exponent.as_ref().clone()))
						.collect::<Vec<_>>();
					*self = mul(new_elements);
					self.optimize_and_reduce();
				}
			},
			Element::Negate(x) => {
				x.optimize_and_reduce();
				if let Some(num) = formula_matches!(x.as_ref(), num(x)) {
					*self = num.neg().into();
					return;
				}
				if let Some(inner) = formula_matches!(x.as_ref(), neg(x)) {
					*self = inner.clone();
				}
			},
		}
	}
}
struct ListElementOptimizationResult {
	add_outer_neg: bool,
}

impl ListElementOptimizationResult {
	fn new() -> Self {
		Self { add_outer_neg: false }
	}
}

fn list_element_optimization(
	elements: &mut Vec<Element>, combine_operation: fn(BigRational, BigRational) -> BigRational,
	inverse_check: fn(&Element, &Element) -> bool, neutral_element_check: fn(&BigRational) -> bool,
	zero_turns_rest_to_zero: bool, is_plus: bool,
) -> ListElementOptimizationResult {
	// inner optimization
	elements.iter_mut().for_each(Element::optimize_and_reduce);

	if let Some(nan) = elements.iter().find(|e| e.is_nan()) {
		*elements = vec![nan.clone()];
		return ListElementOptimizationResult::new();
	}

	// determine the sum of all the rational numbers inside elements
	let combined_rational = elements
		.iter()
		.filter_map(|e| formula_matches!(e, num(x)))
		.filter_map(|n| n.get_exact_rational())
		.reduce(combine_operation)
		.filter(|e| !neutral_element_check(e));

	// collect all non-rational elements
	let mut other_elements = elements
		.iter()
		.filter(|e| formula_matches!(e, num(x)).is_none_or(|e| e.get_exact_rational().is_none()))
		.cloned()
		.collect::<Vec<_>>();

	let mut add_outer_neg = false;
	if !is_plus {
		for e in other_elements.iter_mut() {
			if let Some(neg) = formula_matches!(e, neg(x)) {
				*e = neg.clone();
				add_outer_neg ^= true;
			}
		}
	}

	// rebuild elements
	let mut new_elements = vec![];
	if let Some(mut combined) = combined_rational {
		if add_outer_neg {
			add_outer_neg = false;
			combined = combined.neg();
		}
		new_elements.push(Number::from(combined.clone()).into());
		if zero_turns_rest_to_zero && combined.is_zero() {
			*elements = new_elements;
			return ListElementOptimizationResult::new();
		}
	}
	for e in other_elements {
		match (is_plus, &e) {
			(true, Element::Plus(inner)) => new_elements.extend(inner.iter().cloned()),
			(false, Element::Multiply(inner)) => new_elements.extend(inner.iter().cloned()),
			_ => new_elements.push(e),
		}
	}

	*elements = new_elements;

	remove_inverse_elements(elements, inverse_check);
	ListElementOptimizationResult { add_outer_neg }
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{FormulaStore, create_default_context};

	macro_rules! test {
		($input:expr,$expected:expr) => {{
			let mut input = $input;
			input.optimize_and_reduce();
			assert_eq!(input, $expected);
		}};
	}

	#[test]
	fn test_optimize_new() {
		use crate::formula_short::{inv, mul, num, var};

		// doppelte Negation
		test!(formula!(neg(neg(var("a")))), var("a"));

		// Plus entfernt Nullen komplett
		test!(formula!(plus(num(0), num(0))), num(0));

		// Plus reduziert auf einzelnes Element nach Nullentfernung
		test!(formula!(plus(num(0), var("a"), num(0))), var("a"));

		// Plus mit additivem Inversen ergibt 0
		test!(formula!(plus(var("a"), neg(var("a")))), num(0));

		// Plus mit additiven Inversen und weiterem Term
		test!(formula!(plus(var("a"), neg(var("a")), var("b"))), var("b"));

		// Plus mit mehreren Paaren
		test!(formula!(plus(var("a"), neg(var("a")), var("b"), var("c"), neg(var("c")))), var("b"));

		// Plus propagiert NaN
		let nan_el = Element::Number(Number::nan(None));
		let mut f = formula!(plus(var("x"), nan_el, var("y")));
		f.optimize_and_reduce();
		assert!(f.is_nan());

		// Multiply propagiert NaN
		let nan_el2 = Element::Number(Number::nan(None));
		let mut f = mul([var("x"), nan_el2.clone(), var("y")]);
		f.optimize_and_reduce();
		assert!(f.is_nan());

		// Multiply mit 0 ergibt 0
		test!(formula!(mul(var("a"), num(0), var("b"))), num(0));

		// Multiply entfernt inverses Paar -> 1
		test!(mul([var("a"), inv(var("a"))]), num(1));

		// Multiply entfernt inverses Paar und reduziert auf einzelnes Element
		test!(formula!(mul(var("a"), var("b"), inv(var("a")))), var("b"));

		// Multiply mit 0 dominiert trotz inverser Faktoren
		test!(formula!(mul(num(0), var("a"), inv(var("a")))), num(0));

		// Potenz Basis 1
		test!(formula!(pow(num(1), var("x"))), num(1));

		// Exponent 1
		test!(formula!(pow(var("a"), num(1))), var("a"));

		// 0^-1 = NaN (Division by zero)
		test!(formula!(pow(num(0), num(-1))), Number::nan(Some(Error::DivisionByZero)).into());

		// 1/0 = NaN (Division by zero)
		test!(formula!(mul(num(1), pow(num(0), num(-1)))), Number::nan(Some(Error::DivisionByZero)).into());

		// 0/0 = NaN (Division by zero)
		test!(formula!(mul(num(0), pow(num(0), num(-1)))), Number::nan(Some(Error::DivisionByZero)).into());

		// Zusammenführen verschachtelter Exponenten
		test!(formula!(pow(pow(num(4), num(2)), num(3))), formula!(pow(num(4), num(6))));

		// Mehrfach verschachtelte Exponenten
		test!(formula!(pow(pow(pow(var("a"), num(2)), num(3)), num(4))), formula!(pow(var("a"), num(24))));

		// Exponent 1 verhindert weiteres Kombinieren
		test!(formula!(pow(pow(var("a"), num(2)), num(1))), formula!(pow(var("a"), num(2))));

		// -(-a)*1 + (0+b) + c*(d*0) => a+b,
		let mut f = formula!(plus(
			mul(neg(neg(var("a"))), num(1)),
			plus(num(0), var("b")),
			mul(var("c"), mul(var("d"), num(0)))
		));
		f.optimize_and_reduce();
		assert_eq!(f, formula!(plus(var("a"), var("b"))));

		// x^0 + y*1 => 1 + y
		test!(formula!(plus(pow(var("x"), num(0)), mul(var("y"), num(1)))), formula!(plus(num(1), var("y"))));

		// (a+b)+(c+d) -> a+b+c+d
		test!(
			formula!(plus(plus(var("a"), var("b")), plus(var("c"), var("d")))),
			formula!(plus(var("a"), var("b"), var("c"), var("d")))
		);

		// (a*b)*(c*d) -> a*b*c*d
		test!(
			formula!(mul(mul(var("a"), var("b")), mul(var("c"), var("d")))),
			formula!(mul(var("a"), var("b"), var("c"), var("d")))
		);

		test!(formula!(mul(var("a"), inv(var("a")))), num(1));

		test!(
			formula!(mul(num(33), pow(mul(num(2), num(33)), neg(num(1))))), // 33 * (2*33)^-1
			Number::from(BigRational::from((1.into(), 2.into()))).into()
		);

		test!(formula!(mul(neg(num(2)))), formula!(num(-2)));
		test!(formula!(mul(neg(num(2)), neg(num(2)))), formula!(num(4)));
	}

	#[test]
	fn test_expr_fun_optimization() {
		// pi optimization
		let mut fs = FormulaStore::new_empty();

		fs.define_default_symbols().unwrap();

		fs.quick_eval("sin(2*pi)", 0);
		fs.quick_eval("sin(-2*pi)", 0);
		fs.quick_eval("sin(10*pi)", 0);
		fs.quick_eval("sin(pi)", 0);
		fs.quick_eval("sin(-pi)", 0);
		fs.quick_eval("sin(pi/2)", 1);
		fs.quick_eval("sin(-pi/2)", -1);
		fs.quick_eval2("sin(pi/6)", "0.5");
		fs.quick_eval2("sin(-pi/6)", "-0.5");
		fs.quick_eval2("sin(pi/3)", "sqrt(3)/2");
	}
}
