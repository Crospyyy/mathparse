use crate::parsing::signature::ParamCount;
use astro_float::BigFloat;
use astro_float::ctx::Context;
use num_rational::BigRational;

mod benchmarking;
mod evaluation;
pub(crate) mod operations;
mod optimization;
mod parsing;
mod printing;
mod storing;
mod testing;

pub use astro_float::RoundingMode;
pub use astro_float::ctx::Context as NumberContext;
pub use benchmarking::Benchmark;
pub use operations::create_default_context;
pub use parsing::get_fun_name_end_of_string;
pub use parsing::signature::Signature;
pub use printing::FormattingOptions;
pub use storing::FormulaStore;
pub use storing::Symbol;

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
    FunctionWithExpression {
        arguments: Vec<Element>,
        param_count: ParamCount,
        expression: FunctionExpression,
        debug_name: String,
    },
    /// A number defined by an expression
    NumberWithExpression { fun: fn(&mut Context) -> Number, debug_name: String },
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

#[derive(Clone, Debug, PartialEq)]
pub enum FunctionExpression {
    SingleArgument(fn(&Number, &mut Context) -> Number),
    MultipleArguments(fn(&mut Context, Vec<Number>) -> Number),
}

#[derive(Clone, Debug)]
pub enum Number {
    Rational(BigRational),
    Float(BigFloat),
}

#[allow(unused)]
mod formula_short {
    use crate::{Element, Number};

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
}
