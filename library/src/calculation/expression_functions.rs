use crate::calculation::helper_functions::sin_radians;
use crate::expression_values::{ExpressionFunType, ExpressionNumType, FunctionExpression};
use crate::parsing::signature::ParamCount;
use crate::{Number, only_in_debug};
use astro_float::ctx::Context;
use astro_float::expr;
use std::rc::Rc;

fn pi(ctx: &mut Context) -> Number {
	Number::Float(ctx.const_pi())
}

fn e(ctx: &mut Context) -> Number {
	Number::Float(ctx.const_e())
}

impl ExpressionNumType {
	pub(crate) fn get_function(&self) -> fn(&mut Context) -> Number {
		match self {
			ExpressionNumType::Pi => pi,
			ExpressionNumType::E => e,
		}
	}
}

fn sin_with_radians(num: &Number, ctx: &mut Context) -> Number {
	if let Some(r) = num.get_exact_rational() {
		only_in_debug!(dbg!("calculate sin with rational"));
		sin_radians(&r, ctx)
	} else {
		only_in_debug!(dbg!("calculate sin with float"));
		let float = num.get_float(ctx);
		let mut result = expr!(sin(float * pi / 2), &mut *ctx);
		if float.inexact() {
			result.set_inexact(true)
		}
		Number::from(result)
	}
}

impl ExpressionFunType {
	pub fn get_function(&self) -> FunctionExpression {
		macro_rules! args_check {
			($args:expr,$len:literal, $name:ident) => {
				if $args.len() != $len {
					eprintln!(
						"{} function expects exactly {} arguments, but got {}",
						stringify!($name),
						$len,
						$args.len()
					);
					return Number::nan(None);
				}
			};
		}

		match self {
			ExpressionFunType::Rem => {
				FunctionExpression::multiple_arguments(|args: Vec<Number>, ctx: &mut Context| {
					args_check!(args, 2, Rem);
					Number::rem(&args[0], &args[1], ctx)
				})
			},
			ExpressionFunType::Sin => FunctionExpression::single_argument(Number::sin),
			ExpressionFunType::SinWithRadians => FunctionExpression::single_argument(sin_with_radians),
			ExpressionFunType::Asin => FunctionExpression::single_argument(Number::asin),
			ExpressionFunType::Cos => FunctionExpression::single_argument(Number::cos),
			ExpressionFunType::Acos => FunctionExpression::single_argument(Number::acos),
			ExpressionFunType::Tan => FunctionExpression::single_argument(Number::tan),
			ExpressionFunType::Atan => FunctionExpression::single_argument(Number::atan),
			ExpressionFunType::Floor => FunctionExpression::single_argument(Number::floor),
			ExpressionFunType::Ceil => FunctionExpression::single_argument(Number::ceil),
			ExpressionFunType::Round => FunctionExpression::single_argument(Number::round),
			ExpressionFunType::Abs => FunctionExpression::single_argument(|n, _| n.abs()),
			ExpressionFunType::Log2 => FunctionExpression::single_argument(Number::log2),
			ExpressionFunType::Log10 => FunctionExpression::single_argument(Number::log10),
			ExpressionFunType::Ln => FunctionExpression::single_argument(Number::ln),
			ExpressionFunType::AssertValueRange(range) => {
				let r = *range;
				FunctionExpression::single_argument(move |n, _c| {
					if n.is_in_value_range(r) { n.clone() } else { Number::nan(None) }
				})
			},
			ExpressionFunType::Custom(c) => c.get_fn(),
		}
	}

	pub(crate) fn get_param_count(&self) -> ParamCount {
		match self {
			ExpressionFunType::Rem => ParamCount::Exactly(2),
			ExpressionFunType::Sin
			| ExpressionFunType::Asin
			| ExpressionFunType::Cos
			| ExpressionFunType::Acos
			| ExpressionFunType::Tan
			| ExpressionFunType::Atan
			| ExpressionFunType::Floor
			| ExpressionFunType::Ceil
			| ExpressionFunType::Round
			| ExpressionFunType::Abs
			| ExpressionFunType::Log2
			| ExpressionFunType::Log10
			| ExpressionFunType::Ln
			| ExpressionFunType::SinWithRadians
			| ExpressionFunType::AssertValueRange(_) => ParamCount::Exactly(1),
			ExpressionFunType::Custom(c) => c.param_count,
		}
	}
}
