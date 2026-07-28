use astro_float::BigFloat;
use num_rational::BigRational;
use std::fmt::Debug;

mod benchmarking;
pub(crate) mod calculation;
mod evaluation;
mod latex_conversion;
mod optimization;
mod outer_store_interaction;
mod parsing;
mod printing;
mod storing;
mod testing;

pub use astro_float::RoundingMode;
pub use astro_float::ctx::Context as NumberContext;
pub use benchmarking::Benchmark;
pub use calculation::create_default_context;
use calculation::expression_values::{ExpressionFunType, ExpressionNumType};
pub use evaluation::DynamicResult;
pub use latex_conversion::convert_from_latex_if_needed;
pub use outer_store_interaction::{RunError, RunResult, RunSuccess};
pub use outer_store_interaction::{RunOptions, RunPrecision};
pub use parsing::{Signature, get_fun_name_end_of_string};
pub use printing::{FormattedCalculationOutput, FormattingOptions, ResultStringWithInfo};
pub use storing::{FormulaStore, NamedSymbol, Symbol};

#[derive(Debug, Clone, PartialEq)]
pub enum Element {
	// Unparsed elements
	/// Unparsed group of elements
	Brackets(Vec<Element>),
	/// Unparsed string
	String(String),

	// Unexpanded formula elements
	/// A function with a name and arguments
	Function { name: String, arguments: Vec<Element> },
	/// A variable with a name
	Variable(String),
	/// A variable, which could either be a number or a function
	VariableOrFunction(String),

	// Expanded formula elements
	/// A function with a stored evaluation expression
	FunctionWithExpression { arguments: Vec<Element>, expr_value: ExpressionFunType },
	/// A number defined by an expression
	NumberWithExpression { expr_value: ExpressionNumType },
	/// List of elements to add together
	Plus(Vec<Element>),
	/// List of elements to multiply together
	Multiply(Vec<Element>),
	/// Exponential operation (base^exponent)
	Pow(Box<Element>, Box<Element>),
	/// Negation of an element (e.g., -x)
	Negate(Box<Element>),
	/// A number
	Number(Number),
}

#[derive(Clone, Debug)]
pub enum Number {
	Rational(BigRational),
	Float(BigFloat),
}

#[allow(unused)]
mod formula_short {
	use crate::calculation::expression_values::{ExpressionFunType, ExpressionNumType};
	use crate::{Element, Number};
	use astro_float::Error;

	pub fn nan(error: Option<Error>) -> Element {
		Element::Number(Number::nan(error))
	}

	pub fn num(num: impl ToString) -> Element {
		Element::Number(Number::from_string(num).unwrap())
	}

	pub fn var_or_fun(name: &str) -> Element {
		Element::VariableOrFunction(name.to_string())
	}

	pub fn var(name: impl ToString) -> Element {
		Element::Variable(name.to_string())
	}

	pub fn fun(name: &str, args: impl IntoIterator<Item = Element>) -> Element {
		Element::Function { name: name.to_string(), arguments: args.into_iter().collect() }
	}

	pub fn neg(element: Element) -> Element {
		Element::Negate(Box::new(element))
	}

	pub fn plus(elements: impl IntoIterator<Item = Element>) -> Element {
		Element::Plus(elements.into_iter().collect())
	}

	pub fn mul(elements: impl IntoIterator<Item = Element>) -> Element {
		Element::Multiply(elements.into_iter().collect())
	}

	pub fn pow(base: Element, exponent: Element) -> Element {
		Element::Pow(Box::new(base), Box::new(exponent))
	}

	/// = element^-1
	pub fn inv(element: Element) -> Element {
		pow(element, neg(num("1")))
	}

	pub fn num_expr(value: ExpressionNumType) -> Element {
		Element::NumberWithExpression { expr_value: value }
	}

	pub fn fun_expr(fun: ExpressionFunType, args: impl IntoIterator<Item = Element>) -> Element {
		Element::FunctionWithExpression { arguments: args.into_iter().collect(), expr_value: fun }
	}
}

impl From<Number> for Element {
	fn from(value: Number) -> Self {
		Element::Number(value)
	}
}

#[macro_export]
macro_rules! only_in_debug {
    {$($arg:tt)+} => {
        #[cfg(debug_assertions)]
        {
            $($arg)+
        }
    };
}

#[macro_export]
macro_rules! debug_print {
    ($($arg:tt)*) => {
        only_in_debug! {
            println!("[{}:{}:{}] {}",
                file!(),
                line!(),
                column!(),
                format_args!($($arg)*)
            )
        }
    };
}
