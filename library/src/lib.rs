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
use thiserror::Error;

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
#[derive(Debug, Clone, PartialEq)]
pub enum ElementParsed {
	// Unexpanded formula elements
	/// A function with a name and arguments
	Function { name: String, arguments: Vec<ElementParsed> },
	/// A variable with a name
	Variable(String),
	/// A variable, which could either be a number or a function
	VariableOrFunction(String),

	// Expanded formula elements
	/// A function with a stored evaluation expression
	FunctionWithExpression { arguments: Vec<ElementParsed>, expr_value: ExpressionFunType },
	/// A number defined by an expression
	NumberWithExpression { expr_value: ExpressionNumType },
	/// List of elements to add together
	Plus(Vec<ElementParsed>),
	/// List of elements to multiply together
	Multiply(Vec<ElementParsed>),
	/// Exponential operation (base^exponent)
	Pow(Box<ElementParsed>, Box<ElementParsed>),
	/// Negation of an element (e.g., -x)
	Negate(Box<ElementParsed>),
	/// A number
	Number(Number),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ElementExpanded {
	/// A function with a stored evaluation expression
	FunctionWithExpression { arguments: Vec<ElementExpanded>, expr_value: ExpressionFunType },
	/// A number defined by an expression
	NumberWithExpression { expr_value: ExpressionNumType },
	/// List of elements to add together
	Plus(Vec<ElementExpanded>),
	/// List of elements to multiply together
	Multiply(Vec<ElementExpanded>),
	/// Exponential operation (base^exponent)
	Pow(Box<ElementExpanded>, Box<ElementExpanded>),
	/// Negation of an element (e.g., -x)
	Negate(Box<ElementExpanded>),
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

#[derive(Debug, Clone, PartialEq, Error)]
pub enum ConversionError<'a> {
	/// An unparsed element that requires parsing before conversion
	#[error("cannot convert unparsed element: {0:?}")]
	UnparsedElement(&'a Element),
	/// An unexpanded element that requires expansion before conversion
	#[error("cannot convert unexpanded element: {0:?}")]
	UnexpandedElement(&'a Element),
}

impl From<ElementParsed> for Element {
	fn from(value: ElementParsed) -> Self {
		match value {
			ElementParsed::Function { name, arguments } => {
				Element::Function { name, arguments: arguments.into_iter().map(Element::from).collect() }
			},
			ElementParsed::Variable(name) => Element::Variable(name),
			ElementParsed::VariableOrFunction(name) => Element::VariableOrFunction(name),
			ElementParsed::FunctionWithExpression { arguments, expr_value } => {
				Element::FunctionWithExpression {
					arguments: arguments.into_iter().map(Element::from).collect(),
					expr_value,
				}
			},
			ElementParsed::NumberWithExpression { expr_value } => {
				Element::NumberWithExpression { expr_value }
			},
			ElementParsed::Plus(elements) => Element::Plus(elements.into_iter().map(Element::from).collect()),
			ElementParsed::Multiply(elements) => {
				Element::Multiply(elements.into_iter().map(Element::from).collect())
			},
			ElementParsed::Pow(base, exponent) => {
				Element::Pow(Box::new(Element::from(*base)), Box::new(Element::from(*exponent)))
			},
			ElementParsed::Negate(element) => Element::Negate(Box::new(Element::from(*element))),
			ElementParsed::Number(number) => Element::Number(number),
		}
	}
}

impl From<ElementExpanded> for Element {
	fn from(value: ElementExpanded) -> Self {
		match value {
			ElementExpanded::FunctionWithExpression { arguments, expr_value } => {
				Element::FunctionWithExpression {
					arguments: arguments.into_iter().map(Element::from).collect(),
					expr_value,
				}
			},
			ElementExpanded::NumberWithExpression { expr_value } => {
				Element::NumberWithExpression { expr_value }
			},
			ElementExpanded::Plus(elements) => {
				Element::Plus(elements.into_iter().map(Element::from).collect())
			},
			ElementExpanded::Multiply(elements) => {
				Element::Multiply(elements.into_iter().map(Element::from).collect())
			},
			ElementExpanded::Pow(base, exponent) => {
				Element::Pow(Box::new(Element::from(*base)), Box::new(Element::from(*exponent)))
			},
			ElementExpanded::Negate(element) => Element::Negate(Box::new(Element::from(*element))),
			ElementExpanded::Number(number) => Element::Number(number),
		}
	}
}

impl From<ElementExpanded> for ElementParsed {
	fn from(value: ElementExpanded) -> Self {
		match value {
			ElementExpanded::FunctionWithExpression { arguments, expr_value } => {
				ElementParsed::FunctionWithExpression {
					arguments: arguments.into_iter().map(ElementParsed::from).collect(),
					expr_value,
				}
			},
			ElementExpanded::NumberWithExpression { expr_value } => {
				ElementParsed::NumberWithExpression { expr_value }
			},
			ElementExpanded::Plus(elements) => {
				ElementParsed::Plus(elements.into_iter().map(ElementParsed::from).collect())
			},
			ElementExpanded::Multiply(elements) => {
				ElementParsed::Multiply(elements.into_iter().map(ElementParsed::from).collect())
			},
			ElementExpanded::Pow(base, exponent) => ElementParsed::Pow(
				Box::new(ElementParsed::from(*base)),
				Box::new(ElementParsed::from(*exponent)),
			),
			ElementExpanded::Negate(element) => {
				ElementParsed::Negate(Box::new(ElementParsed::from(*element)))
			},
			ElementExpanded::Number(number) => ElementParsed::Number(number),
		}
	}
}

impl<'a> TryFrom<&'a Element> for ElementParsed {
	type Error = &'a Element;

	fn try_from(value: &'a Element) -> Result<Self, Self::Error> {
		match value {
			Element::Brackets(_) | Element::String(_) => Err(value),
			Element::Function { name, arguments } => Ok(ElementParsed::Function {
				name: name.clone(),
				arguments: arguments.into_iter().map(ElementParsed::try_from).collect::<Result<_, _>>()?,
			}),
			Element::Variable(name) => Ok(ElementParsed::Variable(name.clone())),
			Element::VariableOrFunction(name) => Ok(ElementParsed::VariableOrFunction(name.clone())),
			Element::FunctionWithExpression { arguments, expr_value } => {
				Ok(ElementParsed::FunctionWithExpression {
					arguments: arguments
						.into_iter()
						.map(ElementParsed::try_from)
						.collect::<Result<_, _>>()?,
					expr_value: expr_value.clone(),
				})
			},
			Element::NumberWithExpression { expr_value } => {
				Ok(ElementParsed::NumberWithExpression { expr_value: expr_value.clone() })
			},
			Element::Plus(elements) => Ok(ElementParsed::Plus(
				elements.into_iter().map(ElementParsed::try_from).collect::<Result<_, _>>()?,
			)),
			Element::Multiply(elements) => Ok(ElementParsed::Multiply(
				elements.into_iter().map(ElementParsed::try_from).collect::<Result<_, _>>()?,
			)),
			Element::Pow(base, exponent) => Ok(ElementParsed::Pow(
				Box::new(ElementParsed::try_from(base.as_ref())?),
				Box::new(ElementParsed::try_from(exponent.as_ref())?),
			)),
			Element::Negate(element) => {
				Ok(ElementParsed::Negate(Box::new(ElementParsed::try_from(element.as_ref())?)))
			},
			Element::Number(number) => Ok(ElementParsed::Number(number.clone())),
		}
	}
}
impl<'a> TryFrom<&'a ElementParsed> for ElementExpanded {
	type Error = &'a ElementParsed;

	fn try_from(value: &'a ElementParsed) -> Result<Self, Self::Error> {
		match value {
			ElementParsed::Function { .. }
			| ElementParsed::Variable(..)
			| ElementParsed::VariableOrFunction(..) => Err(value),
			ElementParsed::FunctionWithExpression { arguments, expr_value } => {
				Ok(ElementExpanded::FunctionWithExpression {
					arguments: arguments
						.into_iter()
						.map(ElementExpanded::try_from)
						.collect::<Result<_, _>>()?,
					expr_value: expr_value.clone(),
				})
			},
			ElementParsed::NumberWithExpression { expr_value } => {
				Ok(ElementExpanded::NumberWithExpression { expr_value: expr_value.clone() })
			},
			ElementParsed::Plus(elements) => Ok(ElementExpanded::Plus(
				elements.into_iter().map(ElementExpanded::try_from).collect::<Result<_, _>>()?,
			)),
			ElementParsed::Multiply(elements) => Ok(ElementExpanded::Multiply(
				elements.into_iter().map(ElementExpanded::try_from).collect::<Result<_, _>>()?,
			)),
			ElementParsed::Pow(base, exponent) => Ok(ElementExpanded::Pow(
				Box::new(ElementExpanded::try_from(base.as_ref())?),
				Box::new(ElementExpanded::try_from(exponent.as_ref())?),
			)),
			ElementParsed::Negate(element) => {
				Ok(ElementExpanded::Negate(Box::new(ElementExpanded::try_from(element.as_ref())?)))
			},
			ElementParsed::Number(number) => Ok(ElementExpanded::Number(number.clone())),
		}
	}
}

impl<'a> TryFrom<&'a Element> for ElementExpanded {
	type Error = ConversionError<'a>;

	fn try_from(value: &'a Element) -> Result<Self, Self::Error> {
		Ok(match value {
			Element::Brackets(_) | Element::String(_) => return Err(ConversionError::UnparsedElement(value)),
			Element::Function { .. } | Element::Variable(..) | Element::VariableOrFunction(..) => {
				return Err(ConversionError::UnexpandedElement(value));
			},
			Element::FunctionWithExpression { arguments, expr_value } => {
				ElementExpanded::FunctionWithExpression {
					arguments: arguments
						.into_iter()
						.map(ElementExpanded::try_from)
						.collect::<Result<_, _>>()?,
					expr_value: expr_value.clone(),
				}
			},
			Element::NumberWithExpression { expr_value } => {
				ElementExpanded::NumberWithExpression { expr_value: expr_value.clone() }
			},
			Element::Plus(elements) => ElementExpanded::Plus(
				elements.into_iter().map(ElementExpanded::try_from).collect::<Result<_, _>>()?,
			),
			Element::Multiply(elements) => ElementExpanded::Multiply(
				elements.into_iter().map(ElementExpanded::try_from).collect::<Result<_, _>>()?,
			),
			Element::Pow(base, exponent) => ElementExpanded::Pow(
				Box::new(ElementExpanded::try_from(base.as_ref())?),
				Box::new(ElementExpanded::try_from(exponent.as_ref())?),
			),
			Element::Negate(element) => {
				ElementExpanded::Negate(Box::new(ElementExpanded::try_from(element.as_ref())?))
			},
			Element::Number(number) => ElementExpanded::Number(number.clone()),
		})
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
