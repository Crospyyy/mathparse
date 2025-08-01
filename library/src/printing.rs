use crate::parsing::signature::ParamCount;
use crate::{Element, FunctionExpression, Number};
use astro_float::ctx::Context;
use astro_float::{BigFloat, Radix};
use colored::Colorize;
use num_rational::BigRational;
use std::cmp::Ordering;
use std::fmt::Display;

impl Element {
    pub fn get_string(&self, ctx: &mut Context) -> String {
        Formula::from_element(self, FormattingOptions::default(), false, ctx).to_string()
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

    fn from_element(
        element: &Element, formatting_options: FormattingOptions, mark_unparsed_red: bool, ctx: &mut Context,
    ) -> Self {
        macro_rules! create_formula {
            ($element:expr) => {
                Self::from_element($element, formatting_options, mark_unparsed_red, ctx)
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
            Element::Number(num) => Formula::Number(num.to_string(formatting_options, ctx)),
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

#[derive(Debug)]
pub(super) struct ScientificNumber {
    negative: bool,
    base: Vec<u8>,
    exponent: i64,
}

impl ScientificNumber {
    pub(super) fn new(negative: bool, base: impl Into<Vec<u8>>, exponent: i64) -> Self {
        Self { negative, base: base.into(), exponent }
    }

    pub(super) fn from_big_float(float: &BigFloat, ctx: &mut Context) -> Option<Self> {
        let (sign, numbers, exp) =
            float.convert_to_radix(Radix::Dec, ctx.rounding_mode(), ctx.consts()).ok()?;
        Some(Self { negative: sign.is_negative(), base: numbers, exponent: exp as i64 - 1 })
    }

    fn from_scientific_string(str: &str) -> Option<Self> {
        let (a, b) = str.split_once("e")?;
        let b: i64 = b.parse().ok()?;

        let mut refined_a = a.to_string();

        let negative = refined_a.starts_with('-');
        if negative {
            refined_a.remove(0);
        }

        if refined_a.is_empty() {
            return None; // empty string is not a valid number
        }

        if refined_a.len() == 1 {
            return if let Some(digit) = a.chars().nth(0).unwrap().to_digit(10) {
                Some(Self { negative, base: vec![digit as u8], exponent: b })
            } else {
                None
            };
        }

        if refined_a.chars().nth(1) == Some('.') {
            refined_a.remove(1);
        } else {
            return None; // invalid format
        }

        let numbers: Vec<_> =
            refined_a.chars().map(|c| c.to_digit(10).map(|n| n as _)).collect::<Option<_>>()?;

        Some(Self { negative, base: numbers, exponent: b })
    }

    pub(super) fn to_string(&self, options: FormattingOptions) -> String {
        let round_to_decimals = options.round_to_decimals.max(1);
        let mut exponent = self.exponent;
        let mut rounded = if round_to_decimals >= self.base.len() {
            self.base.clone()
        } else {
            let mut numbers = self.base[..=round_to_decimals].to_vec();
            if numbers[round_to_decimals] >= 5 {
                for i in (0..round_to_decimals).rev() {
                    if numbers[i] == 9 {
                        numbers[i] = 0;
                        if i == 0 {
                            numbers.insert(0, 1);
                            exponent += 1;
                        }
                    } else {
                        numbers[i] += 1;
                        break;
                    }
                }
            }
            numbers.pop();
            numbers
        };
        while rounded.last() == Some(&0) {
            rounded.pop();
        }
        if rounded.is_empty() {
            return "0".to_string();
        }
        if exponent.abs() as usize > options.non_scientific_decimals
            && !(exponent.is_positive() && rounded.len() as i64 > exponent)
        {
            let mut output_string = rounded.iter().map(|n| n.to_string()).collect::<String>();
            if output_string.len() > 1 {
                output_string.insert(1, '.');
            }
            return format!("{}{}e{}", if self.negative { "-" } else { "" }, output_string, exponent,);
        }
        match self.exponent.cmp(&0) {
            Ordering::Less => {
                let mut output_string = vec![0; (-self.exponent) as usize]
                    .into_iter()
                    .chain(rounded.into_iter())
                    .map(|n| n.to_string())
                    .collect::<String>();
                output_string.insert(1, '.');
                if self.negative { format!("-{}", output_string) } else { output_string }
            },
            Ordering::Equal => {
                let mut output_string = rounded.iter().map(|n| n.to_string()).collect::<String>();
                if output_string.len() > 1 {
                    output_string.insert(1, '.');
                }
                if self.negative { format!("-{}", output_string) } else { output_string }
            },
            Ordering::Greater => {
                let add_digits = self.exponent - rounded.len() as i64 + 1;
                let mut output_digits = rounded;
                if add_digits > 0 {
                    output_digits.extend(vec![0; add_digits as usize]);
                }
                let mut output_string = output_digits.iter().map(|n| n.to_string()).collect::<String>();
                if output_string.len() > 1 && add_digits < 0 {
                    output_string.insert((self.exponent + 1) as _, '.');
                }
                if self.negative { format!("-{}", output_string) } else { output_string }
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FormattingOptions {
    pub round_to_decimals: usize,
    pub non_scientific_decimals: usize,
    pub bool_thousands_separator: bool,
}

impl Default for FormattingOptions {
    fn default() -> Self {
        Self { round_to_decimals: 20, non_scientific_decimals: 12, bool_thousands_separator: true }
    }
}

impl FormattingOptions {
    pub fn with_rounding(mut self, decimals: usize) -> Self {
        self.round_to_decimals = decimals;
        self
    }

    pub fn with_non_scientific_decimals(mut self, decimals: usize) -> Self {
        self.non_scientific_decimals = decimals;
        self
    }

    pub fn with_thousands_separator(mut self, bool: bool) -> Self {
        self.bool_thousands_separator = bool;
        self
    }
}

impl From<BigRational> for Number {
    fn from(value: BigRational) -> Self {
        Self::Rational(value)
    }
}
