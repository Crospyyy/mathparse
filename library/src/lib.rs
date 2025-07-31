use crate::parsing::signature::ParamCount;
use astro_float::BigFloat;
use astro_float::ctx::Context;
use num_rational::BigRational;

mod evaluation;
pub mod operations;
pub mod parsing;
mod printing;
pub mod storing;
mod testing;

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
    },
    /// A number defined by an expression
    NumberWithExpression(fn(&mut Context) -> Number),
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

#[derive(Clone, Debug, PartialEq)]
pub enum Number {
    Rational(BigRational),
    Float(BigFloat),
}

#[allow(unused)]
mod formula_short {
    use crate::{Element, Number};

    pub fn num(num: &str) -> Element {
        Element::Number(Number::from_string(num.to_owned()).unwrap())
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
