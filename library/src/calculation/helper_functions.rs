use crate::calculation::helper_functions;
use crate::only_in_debug;
use crate::{Number, debug_print};
use astro_float::ctx::Context;
use astro_float::{BigFloat, Error, expr};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};
use std::str::FromStr;

#[macro_export]
macro_rules! inexact_if_needed {
	($expr:expr, $item:ident) => {{
		let mut result = $expr;
		if $item.inexact() {
			result.set_inexact(true);
		}
		result
	}};
	($expr:expr, $item:ident, $item2:ident) => {{
		if $item.is_nan() {
			$item.clone()
		} else if $item2.is_nan() {
			$item2.clone()
		} else {
			let mut result = $expr;
			if $item.inexact() || $item2.inexact() {
				result.set_inexact(true);
			}
			result
		}
	}};
}

pub(super) fn power_rational_and_rational(
	base: &BigRational, exponent: &BigRational, ctx: &mut Context,
) -> Number {
	macro_rules! safe_return_float_calculation {
		() => {
			let base_float = float_from_rational(base, ctx);
			let exponent_float = float_from_rational(exponent, ctx);
			return Number::from(inexact_if_needed!(
				expr!(pow(base_float, exponent_float), &mut *ctx),
				base_float,
				exponent_float
			));
		};
	}
	if exponent.is_one() {
		// x1/x2 ^ 1 = x1/x2
		return Number::from(base.clone());
	}
	if exponent.is_zero() {
		// x1/x2 ^ 0 = 1
		return Number::from(BigRational::one()); // any number to the power of 0 is 1
	}
	if exponent.is_negative() && base.is_zero() {
		return Number::nan(Some(Error::DivisionByZero));
	}
	if exponent.is_integer() {
		// x1/x2 ^ n = (x1^n)/(x2^n)
		let Some(exp) = exponent.to_i32() else {
			safe_return_float_calculation!();
		};
		return Number::from(base.pow(exp));
	}

	// If we reach here, we have a case like x1/x2 ^ (n/d)
	// We now do the following: (x1/x2 ^ n) ^ (1/d)
	let intermediate = if exponent.numer().is_one() {
		base.clone()
	} else {
		let Some(exp) = exponent.numer().to_i32() else {
			safe_return_float_calculation!();
		};
		base.pow(exp)
	};

	// Now the exponents' numerator is handled, we need to handle the denominator

	let numer_result = big_int_to_power_of_inv_of_big_int(intermediate.numer(), exponent.denom(), ctx);
	let denom_result = big_int_to_power_of_inv_of_big_int(intermediate.denom(), exponent.denom(), ctx);
	numer_result.div(&denom_result, ctx)
}

/// Calculates the result of a^(1/b).
/// If a precise result is not possible, it returns a float.
pub(super) fn big_int_to_power_of_inv_of_big_int(a: &BigInt, b: &BigInt, ctx: &mut Context) -> Number {
	let float_a = BigFloat::from_str(&a.to_string()).unwrap();
	let float_b = BigFloat::from_str(&b.to_string()).unwrap();
	let result = if b == &2.into() {
		expr!(sqrt(float_a), &mut *ctx)
	} else {
		expr!(pow(float_a, 1 / float_b), &mut *ctx)
	};

	if ctx.precision() < 164 || result.is_nan() || result.is_inf() {
		return Number::from(result);
	} // if precision is too low rounding is not possible

	let rounding_precision = (ctx.precision() - 100).min((ctx.precision() * 8) / 10);

	let result_rounded = result.round(rounding_precision, ctx.rounding_mode());

	// unwrap is safe here because b is neither nan nor infinity
	let result_rational = rational_from_float(&result_rounded).unwrap();
	let Some(b_i32) = b.to_i32() else {
		return if expr!(pow(result_rounded, float_b), &mut *ctx) == float_a {
			let mut made_exact = result_rounded;
			made_exact.set_inexact(false);
			Number::from(made_exact)
		} else {
			Number::from(result)
		};
	};
	let converted_back = result_rational.pow(b_i32);
	if converted_back.is_integer() && converted_back.numer() == a {
		Number::from(result_rational)
	} else {
		Number::from(result)
	}
}

/// Converts a `BigFloat` to a `BigRational` if it is exact.
/// Returns `None` if the float is inexact or cannot be converted.
pub fn float_to_exact_rational(float: &BigFloat) -> Option<BigRational> {
	if float.inexact() {
		return None;
	}
	rational_from_float(float)
}

pub(super) fn rational_from_string(s: &str) -> Option<BigRational> {
	BigRational::from_str(s).ok()
}

pub(super) fn float_from_rational(rational: &BigRational, ctx: &mut Context) -> BigFloat {
	let a_float = BigFloat::from_str(&rational.numer().to_string()).unwrap();
	let b_float = BigFloat::from_str(&rational.denom().to_string()).unwrap();
	expr!(a_float / b_float, &mut *ctx)
}

pub(crate) fn rational(n: i32) -> BigRational {
	BigRational::from_integer(n.into())
}

/// Returns `None` if the float is inf or NaN.
pub(super) fn rational_from_float(float: &BigFloat) -> Option<BigRational> {
	if *float == BigFloat::from(1) {
		return Some(rational(1));
	}
	use num_bigint::Sign as IntSign;
	let sign_positive = float.sign()?.is_positive();
	let exponent = float.exponent()?;
	let mantissa = float.mantissa_digits()?;
	let bytes: Vec<u8> = mantissa.iter().rev().flat_map(|v| v.to_be_bytes().into_iter()).collect();
	let numerator = BigRational::from_integer(BigInt::from_bytes_be(
		if sign_positive { IntSign::Plus } else { IntSign::Minus },
		&bytes,
	));
	let exp_adj = exponent - (size_of_val(mantissa) * 8) as i32;
	let ratio = numerator * BigRational::from_integer(2.into()).pow(exp_adj);
	Some(ratio)
}

/// r = radians (0..1 = top right, 1..2 = top left, 2..3 = bottom left, 3..4 = bottom right)
pub(crate) fn sin_radians(r: &BigRational, ctx: &mut Context) -> Number {
	dbg!(&r);
	let mut r = r.clone() % rational(4);
	if r.is_negative() {
		r += rational(4);
	}
	dbg!(&r);
	let mapped = if r < rational(1) {
		r
	} else if r < rational(3) {
		rational(2) - r
	} else {
		r - rational(4)
	};

	fn negate_if(num: i32, neg: bool) -> i32 {
		if neg { -num } else { num }
	}
	if mapped == rational(0) {
		debug_print!("returning zero");
		return Number::from(0);
	}
	if mapped.abs() == rational(1) {
		debug_print!("returning one");
		return Number::from(negate_if(1, mapped.is_negative()));
	}
	if mapped.abs() == BigRational::new(1.into(), 3.into()) {
		debug_print!("returning 1/2");
		return Number::Rational(BigRational::new(negate_if(1, mapped.is_negative()).into(), 2.into()));
	}
	if mapped.abs() == BigRational::new(2.into(), 3.into()) {
		debug_print!("returning 1/2");
		let negator = negate_if(1, mapped.is_negative());
		return Number::from(expr!(negator * sqrt(3) / 2, &mut *ctx));
	}
	
	debug_print!("return sin calculation");
	let float = helper_functions::float_from_rational(&(mapped / rational(2)), ctx);
	let sin_input = expr!(float * pi, &mut *ctx);
	let float = expr!(sin(sin_input), &mut *ctx);
	Number::from(float)
}
