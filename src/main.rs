mod evaluation;
mod parsing;
mod printing;

fn main() {}

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
