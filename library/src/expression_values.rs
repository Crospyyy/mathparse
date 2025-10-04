use crate::Number;
use crate::parsing::signature::ParamCount;
use astro_float::ctx::Context;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ExpressionFunType {
	Custom(CustomFunction),
	Sin,
	SinWithRadians,
	Asin,
	Cos,
	Acos,
	Tan,
	Atan,
	Floor,
	Ceil,
	Round,
	Abs,
	Rem,
	Log2,
	Log10,
	Ln,
	AssertValueRange(ValueRange),
}

impl From<CustomFunction> for ExpressionFunType {
	fn from(value: CustomFunction) -> Self {
		ExpressionFunType::Custom(value)
	}
}

#[derive(Clone, Debug)]
pub(crate) struct CustomFunction {
	pub(crate) name: &'static str,
	pub(crate) param_count: ParamCount,
	function: FunctionExpression,
}

impl CustomFunction {
	pub(crate) fn new(name: &'static str, param_count: ParamCount, function: FunctionExpression) -> Self {
		CustomFunction { name, param_count, function }
	}

	pub(crate) fn single_argument(
		name: &'static str, param_count: ParamCount, f: impl Fn(&Number, &mut Context) -> Number + 'static,
	) -> Self {
		CustomFunction::new(name, param_count, FunctionExpression::single_argument(f))
	}
	pub(crate) fn multiple_arguments(
		name: &'static str, param_count: ParamCount,
		f: impl Fn(Vec<Number>, &mut Context) -> Number + 'static,
	) -> Self {
		CustomFunction::new(name, param_count, FunctionExpression::multiple_arguments(f))
	}

	pub fn get_fn(&self) -> FunctionExpression {
		self.function.clone()
	}
}

impl PartialEq for CustomFunction {
	/// This always returns false since custom functions can't be compared
	fn eq(&self, _other: &Self) -> bool {
		false
	}
}

#[derive(Clone)]
pub enum FunctionExpression {
	SingleArgument(Rc<dyn Fn(&Number, &mut Context) -> Number>),
	MultipleArguments(Rc<dyn Fn(Vec<Number>, &mut Context) -> Number>),
}

impl Debug for FunctionExpression {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			FunctionExpression::SingleArgument(_) => f.write_str("SingleArgument"),
			FunctionExpression::MultipleArguments(_) => f.write_str("MultipleArguments"),
		}
	}
}

impl FunctionExpression {
	pub(crate) fn single_argument(f: impl Fn(&Number, &mut Context) -> Number + 'static) -> Self {
		FunctionExpression::SingleArgument(Rc::new(f))
	}
	pub(crate) fn multiple_arguments(f: impl Fn(Vec<Number>, &mut Context) -> Number + 'static) -> Self {
		FunctionExpression::MultipleArguments(Rc::new(f))
	}
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ValueRange {
	Positive,
	Negative,
	PositiveOrZero,
	NegativeOrZero,
	NotZero,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ExpressionNumType {
	Pi,
	E,
}
