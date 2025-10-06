use super::*;
use crate::calculation::helper_functions::power_rational_and_rational;
use crate::inexact_if_needed;
use num_bigint::BigInt;

/// Macro to implement operations that convert the number to a float and apply the operation
macro_rules! float_op {
	($op:ident) => {
		pub fn $op(&self, ctx: &mut Context) -> Self {
			let float = self.get_float(ctx);
			Number::from(inexact_if_needed!(expr!($op(float), &mut *ctx), float))
		}
	};
}

// implement operations where num needs to be converted to a float
impl Number {
	float_op!(sin);
	float_op!(asin);
	float_op!(cos);
	float_op!(acos);
	float_op!(tan);
	float_op!(atan);

	float_op!(log2); // todo check if this can be implemented with rational numbers
	float_op!(log10);
	float_op!(ln);
}

// implement operations for just one number
impl Number {
	pub fn neg(&self) -> Self {
		match self {
			Self::Rational(r) => Self::from(-r),
			Self::Float(f) => Self::from(inexact_if_needed!(f.neg(), f)),
		}
	}

	pub fn abs(&self) -> Self {
		if let Some(r) = self.get_exact_rational() {
			return Self::from(r.abs());
		}
		match self {
			Number::Float(f) => Self::from(inexact_if_needed!(f.abs(), f)),
			_ => unreachable!(),
		}
	}

	pub fn floor(&self, ctx: &mut Context) -> Self {
		if let Some(r) = self.get_exact_rational() {
			Self::from(r.floor())
		} else {
			let float = self.get_float(ctx);
			Self::from(inexact_if_needed!(float.floor(), float))
		}
	}
	
	pub fn fac(&self, ctx: &mut Context) -> Self {
		if let Some(r) = self.get_exact_rational()
			&& r.is_integer()
			&& let Some(n) = r.numer().to_i32()
		{
			if n < 0 {
				return Self::nan(None); // factorial is not defined for negative numbers
			}
			if n > 10000 {
				return Self::nan(None); // prevent extremely long calculations
			}
			let mut result = BigInt::from(1);
			for i in 1..=n {
				result *= BigInt::from(i);
			}
			Self::from(BigRational::from_integer(result))
		} else {
			Self::nan(None)
		}
	}

	pub fn ceil(&self, ctx: &mut Context) -> Self {
		if let Some(r) = self.get_exact_rational() {
			Self::from(r.ceil())
		} else {
			let float = self.get_float(ctx);
			Self::from(inexact_if_needed!(float.ceil(), float))
		}
	}
	pub fn round(&self, ctx: &mut Context) -> Self {
		if let Some(r) = self.get_exact_rational() {
			Self::from(r.round())
		} else {
			let float = self.get_float(ctx);
			let ceil = float.ceil();
			let diff_ceil = float.sub(&ceil, ctx.precision(), ctx.rounding_mode()).abs();

			match diff_ceil.partial_cmp(&BigFloat::from(0.5)) {
				None => {
					Self::nan(None) // cannot compare, return NaN
				},
				Some(Ordering::Less) | Some(Ordering::Equal) => Self::from(inexact_if_needed!(ceil, float)),
				Some(Ordering::Greater) => Self::from(inexact_if_needed!(float.floor(), float)),
			}
		}
	}
}

// implement operations where there are two numbers as parameters
impl Number {
	pub fn rem(&self, other: &Self, ctx: &mut Context) -> Self {
		if let (Number::Rational(r1), Number::Rational(r2)) = (self, other) {
			if r2.is_zero() {
				return Self::nan(None);
			}
			let mut result = r1 % r2;
			if result.is_negative() {
				result += r2;
			}
			Self::from(result)
		} else {
			let self_float = self.get_float(ctx);
			let other_float = other.get_float(ctx);
			Self::from(inexact_if_needed!(
				expr!(self_float % other_float, &mut *ctx),
				self_float,
				other_float
			))
		}
	}

	pub fn plus(&self, other: &Self, ctx: &mut Context) -> Self {
		if let (Some(a), Some(b)) = (self.get_exact_rational(), other.get_exact_rational()) {
			Self::from(a + b)
		} else {
			let (a, b) = (self.get_float(ctx), other.get_float(ctx));
			Self::from(inexact_if_needed!(expr!(a + b, &mut *ctx), a, b))
		}
	}

	pub fn mul(&self, other: &Self, ctx: &mut Context) -> Self {
		if let (Some(a), Some(b)) = (self.get_exact_rational(), other.get_exact_rational()) {
			Self::from(a * b)
		} else {
			if self.is_nan() {
				return self.clone();
			} else if other.is_nan() {
				return other.clone();
			}
			let (a, b) = (self.get_float(ctx), other.get_float(ctx));
			Self::from(inexact_if_needed!(expr!(a * b, &mut *ctx), a, b))
		}
	}

	pub fn div(&self, other: &Self, ctx: &mut Context) -> Self {
		if let (Some(a), Some(b)) = (self.get_exact_rational(), other.get_exact_rational()) {
			if b.is_zero() {
				return Self::nan(None); // handle division by zero
			}
			return Self::from(a / b);
		}
		let (a, b) = (self.get_float(ctx), other.get_float(ctx));
		if b.is_zero() {
			return Self::nan(None); // handle division by zero
		}
		Self::from(inexact_if_needed!(expr!(a / b, &mut *ctx), a, b))
	}

	pub fn pow(&self, other: &Self, ctx: &mut Context) -> Self {
		if let (Some(a), Some(b)) = (self.get_exact_rational(), other.get_exact_rational()) {
			return power_rational_and_rational(&a, &b, ctx);
		}
		if let (Some(a), Some(b)) = (
			self.get_exact_rational(),
			other.get_exact_rational().and_then(|r| {
				if !r.is_integer() {
					return None;
				}
				r.to_i32()
			}),
		) {
			return Self::from(a.pow(b));
		}

		let (a, b) = (self.get_float(ctx), other.get_float(ctx));
		Self::from(inexact_if_needed!(expr!(pow(a, b), &mut *ctx), a, b))
	}

	pub fn max(&self, other: &Self, ctx: &mut Context) -> Self {
		let Some(comp) = self.cmp(other, ctx) else { return Self::nan(None) };
		if comp.is_ge() { self.clone() } else { other.clone() }
	}

	pub fn min(&self, other: &Self, ctx: &mut Context) -> Self {
		let Some(comp) = self.cmp(other, ctx) else { return Self::nan(None) };
		if comp.is_le() { self.clone() } else { other.clone() }
	}
}

// implement operations where there is a list of numbers as a parameter
impl Number {
	pub fn sum(numbers: &[Self], ctx: &mut Context) -> Self {
		numbers.iter().fold(Number::from(0), |a, b| a.plus(b, ctx))
	}

	/// Returns `None` if numbers is an empty array
	pub fn average(numbers: &[Self], ctx: &mut Context) -> Option<Self> {
		match numbers.len() {
			0 => return None,
			1 => return Some(numbers[0].clone()),
			_ => {},
		}
		Some(Self::sum(numbers, ctx).div(&Number::from(numbers.len()), ctx))
	}

	/// Returns `None` if numbers is an empty array
	pub fn median(numbers: &[Self], ctx: &mut Context) -> Option<Self> {
		let num_args = numbers.len();
		match num_args {
			0 => return None,
			1 => return Some(numbers[0].clone()),
			_ => {},
		}
		if numbers.iter().any(|num| num.is_nan()) {
			return Some(Self::nan(None));
		}
		let mut sorted = numbers.to_vec();
		sorted.sort_by(|a, b| a.cmp(b, ctx).unwrap());
		let middle = num_args / 2;
		Some(if num_args % 2 == 1 {
			sorted[middle].clone()
		} else {
			sorted[middle - 1].plus(&sorted[middle], ctx).div(&Number::from_string("2").unwrap(), ctx)
		})
	}

	/// Returns `None` if numbers is an empty array
	pub fn max_of_several(numbers: &[Self], ctx: &mut Context) -> Option<Self> {
		let num_args = numbers.len();
		match num_args {
			0 => return None,
			1 => return Some(numbers[0].clone()),
			_ => {},
		}
		if numbers.iter().any(|num| num.is_nan()) {
			return Some(Self::nan(None));
		}
		Some(numbers.iter().max_by(|a, b| a.cmp(b, ctx).unwrap()).unwrap().clone())
	}

	/// Returns `None` if numbers is an empty array
	pub fn min_of_several(numbers: &[Self], ctx: &mut Context) -> Option<Self> {
		let num_args = numbers.len();
		match num_args {
			0 => return None,
			1 => return Some(numbers[0].clone()),
			_ => {},
		}
		if numbers.iter().any(|num| num.is_nan()) {
			return Some(Self::nan(None));
		}
		Some(numbers.iter().min_by(|a, b| a.cmp(b, ctx).unwrap()).unwrap().clone())
	}
}
