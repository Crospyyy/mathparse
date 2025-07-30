use crate::parsing::signature::ParamCount;
use crate::{Element, FunctionExpression, Number};
use astro_float::ctx::Context;
use colored::Colorize;
use std::fmt::Display;

impl Element {
    pub fn get_string(&self, ctx: &mut Context) -> String {
        Formula::from_element(self, Number::DEFAULT_ROUNDING_DIGITS, false, ctx).to_string()
    }

    pub fn get_debug_string(&self) -> String {
        match self {
            Element::Brackets(e) => {
                format!("br({})", e.iter().map(Self::get_debug_string).collect::<Vec<_>>().join(", "))
            },
            Element::Plus(e) => {
                format!("add({})", e.iter().map(Self::get_debug_string).collect::<Vec<_>>().join(", "))
            },
            Element::Multiply(e) => {
                format!("mul({})", e.iter().map(Self::get_debug_string).collect::<Vec<_>>().join(", "))
            },
            Element::Pow(a, b) => format!("pow({}, {})", a.get_debug_string(), b.get_debug_string()),
            Element::String(s) => format!("\"{s}\""),
            Element::Negate(e) => format!("neg({})", e.get_debug_string()),
            Element::Number(n) => format!("num({})", n.get_debug_string()),
            Element::Function { name, arguments } => {
                format!(
                    "fun({name}, [{}])",
                    arguments.iter().map(Self::get_debug_string).collect::<Vec<_>>().join(", ")
                )
            },
            Element::Variable(name) => format!("var({})", name),
            Element::VariableOrFunction(name) => {
                format!("var_or_fun({})", name)
            },
            Element::FunctionWithExpression { arguments, expression, param_count } => {
                let param_count_str = match param_count {
                    ParamCount::Exactly(n) => format!("={n} params"),
                    ParamCount::AtLeast(n) => format!(">={n} params"),
                };
                let name_str = if matches!(expression, FunctionExpression::SingleArgument(_)) {
                    "one argument"
                } else {
                    "n arguments"
                };
                format!(
                    "fun_with_expr({}, {}, [{}])",
                    param_count_str,
                    name_str,
                    arguments.iter().map(Self::get_debug_string).collect::<Vec<_>>().join(", ")
                )
            },
            Element::NumberWithExpression(_) => {
                format!("num_with_expr({})", self.get_debug_string())
            },
        }
    }
}

pub enum Formula {
    Plus(Vec<Formula>),
    Multiply(Vec<Formula>),
    Negate(Box<Formula>),
    Number(String),
    Variable(String),
    Pow(Box<Formula>, Box<Formula>),
    Division(Box<Formula>, Box<Formula>),
    Function { name: String, arguments: Vec<Formula> },
    ForcedBrackets(Vec<Self>),
}

impl Formula {
    fn get_priority(&self) -> usize {
        match self {
            Formula::Plus(_) => 0,
            Formula::Multiply(_) | Formula::Division(..) | Formula::Negate(_) => 1,
            Formula::Pow(..) => 2,
            Formula::Number(_)
            | Formula::Variable(_)
            | Formula::Function { .. }
            | Formula::ForcedBrackets(_) => 3,
        }
    }
}

impl Display for Formula {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Formula::Plus(elements) => {
                write!(f, "{}", elements.iter().map(|e| format!("{}", e)).collect::<Vec<_>>().join(" + "))
            },
            Formula::Multiply(elements) => {
                let string = elements
                    .iter()
                    .map(|e| {
                        if e.get_priority() < self.get_priority() {
                            format!("({})", e)
                        } else {
                            e.to_string()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" * ");
                write!(f, "{}", string)
            },
            Formula::Negate(e) => {
                if matches!(e.as_ref(), Formula::Plus(..) | Formula::Multiply(..)) {
                    write!(f, "-({})", e)
                } else {
                    write!(f, "-{}", e)
                }
            },
            Formula::Number(n) | Formula::Variable(n) => write!(f, "{}", n),
            Formula::Pow(base, exponent) => {
                if base.get_priority() <= self.get_priority() {
                    write!(f, "({})^", base)?;
                } else {
                    write!(f, "{}^", base)?;
                }
                if exponent.get_priority() < self.get_priority()
                    && !matches!(exponent.as_ref(), Formula::Negate(_))
                {
                    write!(f, "({})", exponent)
                } else {
                    write!(f, "{}", exponent)
                }
            },
            Formula::Division(numerator, denominator) => {
                if numerator.get_priority() < self.get_priority() {
                    write!(f, "({})/", numerator)?;
                } else {
                    write!(f, "{}/", numerator)?;
                }
                if denominator.get_priority() <= self.get_priority() {
                    write!(f, "({})", denominator)
                } else {
                    write!(f, "{}", denominator)
                }
            },
            Formula::Function { name, arguments } => {
                write!(
                    f,
                    "{}({})",
                    name,
                    arguments.iter().map(|a| format!("{}", a)).collect::<Vec<_>>().join(", ")
                )
            },
            Formula::ForcedBrackets(elements) => {
                write!(
                    f,
                    "({})",
                    elements.iter().map(Self::to_string).reduce(|a, b| a + &b).unwrap_or_default()
                )
            },
        }
    }
}

impl Formula {
    fn from_element(
        element: &Element, rounding_digits: usize, mark_unparsed_red: bool, ctx: &mut Context,
    ) -> Self {
        macro_rules! create_formula {
            ($element:expr) => {
                Self::from_element($element, rounding_digits, mark_unparsed_red, ctx)
            };
        }

        match element {
            Element::Plus(elements) => {
                let elements = elements.iter().map(|e| create_formula!(e)).collect();
                Formula::Plus(elements)
            },
            Element::Multiply(elements) => {
                let elements = elements.iter().map(|e| create_formula!(e)).collect();
                Formula::Multiply(elements)
            },
            Element::Negate(e) => Formula::Negate(Box::new(create_formula!(e))),
            Element::Number(num) => Formula::Number(num.to_string(rounding_digits, ctx)),
            Element::Variable(name) => Formula::Variable(name.clone()),
            Element::VariableOrFunction(name) => Formula::Variable(name.clone()),
            Element::Pow(base, exponent) => {
                Formula::Pow(Box::new(create_formula!(base)), Box::new(create_formula!(exponent)))
            },
            Element::Function { name, arguments } => Formula::Function {
                name: name.clone(),
                arguments: arguments.iter().map(|e| create_formula!(e)).collect(),
            },

            Element::Brackets(elements) => {
                Formula::ForcedBrackets(elements.iter().map(|e| create_formula!(e)).collect())
            },
            Element::String(s) => {
                Formula::Variable(if mark_unparsed_red { s.red().to_string() } else { s.to_string() })
            },
            Element::FunctionWithExpression { arguments, .. } => Formula::Function {
                name: "fun_with_expr".to_string(),
                arguments: arguments.iter().map(|e| create_formula!(e)).collect(),
            },
            Element::NumberWithExpression(_) => Formula::Variable("num_with_expr".to_string()),
        }
    }
}
