mod application;
mod libraries;

#[cfg(not(test))]
mod gui_application;

#[cfg(not(test))]
fn main() {
    gui_application::main();
    // run_formula_evaluator();
}

#[derive(Debug, Clone, PartialEq)]
pub enum Element {
    /// Unparsed group of elements
    Brackets(Vec<Element>),
    /// Unparsed string
    String(String),

    /// List of elements to add together
    Plus(Vec<Element>),
    /// List of elements to multiply together
    Multiply(Vec<Element>),
    /// Exponential operation (base^exponent)
    Pow(Box<Element>, Box<Element>),
    /// Negation of an element (e.g., -x)
    Negate(Box<Element>),

    /// A number
    Number(f64),
    /// A function with a name and arguments
    Function { name: String, arguments: Vec<Element> },
    /// A variable with a name
    Variable(String),
    /// A variable, which could either be a number or a function
    VariableOrFunction(String),
}

mod formula_short {
    use crate::Element;

    pub fn num(num: f64) -> Element {
        Element::Number(num)
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

    pub fn inv(element: Element) -> Element {
        pow(element, neg(num(1.0)))
    }
}
