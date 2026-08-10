use crate::calculation::helper_functions::float_to_exact_rational;
use crate::{ElementParsed, Number};
use astro_float::BigFloat;
use macros::formula_matches;
use num_rational::BigRational;
use num_traits::ToPrimitive;

impl From<BigRational> for Number {
	fn from(value: BigRational) -> Self {
		Self::Rational(value)
	}
}

impl From<BigFloat> for Number {
	fn from(value: BigFloat) -> Self {
		if let Some(rational) = float_to_exact_rational(&value) {
			Self::Rational(rational)
		} else {
			Self::Float(value)
		}
	}
}

impl From<usize> for Number {
	fn from(value: usize) -> Self {
		Self::Rational(BigRational::from_integer(value.into()))
	}
}

impl From<i32> for Number {
	fn from(value: i32) -> Self {
		Self::Rational(BigRational::from_integer(value.into()))
	}
}

impl PartialEq for Number {
	fn eq(&self, other: &Self) -> bool {
		match (self, other) {
			(Number::Rational(a), Number::Rational(b)) => a == b,
			(Number::Float(a), Number::Float(b)) => {
				a.eq(b) || (a.is_nan() && b.is_nan() && (a.err() == b.err()))
			},
			_ => false,
		}
	}
}

impl PartialEq<i32> for Number {
	fn eq(&self, other: &i32) -> bool {
		match self {
			Number::Rational(r) => r.is_integer() && r.to_i32() == Some(*other),
			Number::Float(f) => f.is_int() && f == &BigFloat::from(*other),
		}
	}
}

impl PartialEq<i32> for &Number {
	fn eq(&self, other: &i32) -> bool {
		match self {
			Number::Rational(r) => r.is_integer() && r.to_i32() == Some(*other),
			Number::Float(f) => f.is_int() && f == &BigFloat::from(*other),
		}
	}
}

impl PartialEq<i32> for &ElementParsed {
	fn eq(&self, other: &i32) -> bool {
		formula_matches!(self, num(x)).is_some_and(|n| n == other)
	}
}
