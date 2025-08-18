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
            Element::FunctionWithExpression { arguments, expression, param_count, debug_name } => {
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
                    "fun_with_expr:{}({}, {}, [{}])",
                    debug_name,
                    param_count_str,
                    name_str,
                    arguments.iter().map(Self::get_debug_string).collect::<Vec<_>>().join(", ")
                )
            },
            Element::NumberWithExpression { debug_name, .. } => format!("num_with_expr:{}", debug_name),
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
            Element::FunctionWithExpression { arguments, debug_name, .. } => Formula::Function {
                name: format!("fun_expr:{debug_name}"),
                arguments: arguments.iter().map(|e| create_formula!(e)).collect(),
            },
            Element::NumberWithExpression { debug_name, .. } => {
                Formula::Variable(format!("num_expr:{debug_name}", ))
            },
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
        let mut modified_exponent = self.exponent;
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
                            modified_exponent += 1;
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
        if Self::should_print_scientific(&rounded, modified_exponent, options) {
            Self::create_scientific_string(&rounded, modified_exponent, self.negative)
        } else {
            Self::create_regular_string(rounded, modified_exponent, self.negative, options)
        }
    }

    fn create_regular_string(
        rounded: Vec<u8>, modified_exponent: i64, negative: bool, formatting: FormattingOptions,
    ) -> String {
        match modified_exponent.cmp(&0) {
            Ordering::Less => {
                let mut output_string = vec![0; (-modified_exponent) as usize]
                    .into_iter()
                    .chain(rounded.into_iter())
                    .map(|n| n.to_string())
                    .collect::<String>();
                output_string.insert(1, '.');
                if negative { format!("-{}", output_string) } else { output_string }
            },
            Ordering::Equal => {
                let mut output_string = rounded.iter().map(|n| n.to_string()).collect::<String>();
                if output_string.len() > 1 {
                    output_string.insert(1, '.');
                }
                if negative { format!("-{}", output_string) } else { output_string }
            },
            Ordering::Greater => {
                let add_digits = modified_exponent - rounded.len() as i64 + 1;
                let mut output_digits = rounded;
                if add_digits > 0 {
                    output_digits.extend(vec![0; add_digits as usize]);
                }
                let mut output_string = output_digits.iter().map(|n| n.to_string()).collect::<String>();
                if output_string.len() > 1 && add_digits < 0 {
                    output_string.insert((modified_exponent + 1) as _, '.');
                }

                if formatting.thousands_separator {
                    let full_digits = (modified_exponent as usize + 1).min(output_digits.len());
                    let (a, b) = output_string.split_at(full_digits);
                    let mut a = a.to_string();
                    for i in (1..a.len().div_ceil(3)).rev() {
                        let pos = a.len() - 3 * i;
                        a.insert(pos, ',');
                    }
                    output_string = format!("{}{}", a, b);
                }

                if negative { format!("-{}", output_string) } else { output_string }
            },
        }
    }

    fn should_print_scientific(rounded: &Vec<u8>, exponent: i64, options: FormattingOptions) -> bool {
        exponent.abs() as usize > options.non_scientific_decimals
            && !(exponent.is_positive() && rounded.len() as i64 > exponent)
    }

    fn create_scientific_string(rounded: &[u8], modified_exponent: i64, negative: bool) -> String {
        let mut output_string = rounded.iter().map(|n| n.to_string()).collect::<String>();
        if output_string.len() > 1 {
            output_string.insert(1, '.');
        }
        format!("{}{output_string}e{}", if negative { "-" } else { "" }, modified_exponent)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FormattingOptions {
    pub round_to_decimals: usize,
    pub non_scientific_decimals: usize,
    pub thousands_separator: bool,
}

impl Default for FormattingOptions {
    fn default() -> Self {
        Self { round_to_decimals: 20, non_scientific_decimals: 12, thousands_separator: true }
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
        self.thousands_separator = bool;
        self
    }
}

#[cfg(test)]
mod tests {
    use crate::FormattingOptions;
    use crate::printing::ScientificNumber;

    #[test]
    fn test_scientific_number() {
        fn quick_conversion(base: Vec<u8>, exponent: i64, no_sci_digits: usize) -> String {
            ScientificNumber::new(false, base, exponent).to_string(
                FormattingOptions::default().with_rounding(100).with_non_scientific_decimals(no_sci_digits),
            )
        }
        assert_eq!(quick_conversion(vec![0, 0, 0], 0, 100), "0");
        assert_eq!(quick_conversion(vec![0, 0, 0], 3, 100), "0");
        assert_eq!(quick_conversion(vec![1], -1, 100), "0.1");
        assert_eq!(quick_conversion(vec![1], 0, 100), "1");
        assert_eq!(quick_conversion(vec![1], 3, 100), "1,000");

        assert_eq!(quick_conversion(vec![1, 2, 3], -2, 100), "0.0123");
        assert_eq!(quick_conversion(vec![1, 2, 3], -1, 100), "0.123");
        assert_eq!(quick_conversion(vec![1, 2, 3], 0, 100), "1.23");
        assert_eq!(quick_conversion(vec![1, 2, 3], 1, 100), "12.3");
        assert_eq!(quick_conversion(vec![1, 2, 3], 2, 100), "123");
        assert_eq!(quick_conversion(vec![1, 2, 3], 3, 100), "1,230");
        assert_eq!(quick_conversion(vec![1, 2, 3], 4, 100), "12,300");
        assert_eq!(quick_conversion(vec![1], 3, 100), "1,000");
        assert_eq!(quick_conversion(vec![1], 6, 100), "1,000,000");
        assert_eq!(quick_conversion(vec![1], 9, 100), "1,000,000,000");

        assert_eq!(quick_conversion(vec![1, 2, 3], -2, 0), "1.23e-2");
        assert_eq!(quick_conversion(vec![1, 2, 3], -1, 0), "1.23e-1");
        assert_eq!(quick_conversion(vec![1, 2, 3], 0, 0), "1.23");
        assert_eq!(quick_conversion(vec![1, 2, 3], 1, 0), "12.3"); // no e because all digits are visible till zero
        assert_eq!(quick_conversion(vec![1, 2, 3], 2, 0), "123"); // no e because all digits are visible till zero
        assert_eq!(quick_conversion(vec![1, 2, 3], 3, 0), "1.23e3");

        assert_eq!(quick_conversion(vec![1, 2, 3], -2, 1), "1.23e-2");
        assert_eq!(quick_conversion(vec![1, 2, 3], -1, 1), "0.123");
        assert_eq!(quick_conversion(vec![1, 2, 3], 0, 1), "1.23");
        assert_eq!(quick_conversion(vec![1, 2, 3], 1, 1), "12.3");
        assert_eq!(quick_conversion(vec![1, 2, 3], 2, 1), "123"); // no e because all digits are visible till zero
        fn quick_round(base: Vec<u8>, round_to_decimals: usize) -> String {
            ScientificNumber::new(false, base, 0)
                .to_string(FormattingOptions::default().with_rounding(round_to_decimals))
        }
        assert_eq!(quick_round(vec![1, 2, 3, 4, 5, 6, 7, 8, 9], 1), "1");
        assert_eq!(quick_round(vec![1, 2, 3, 4, 5, 6, 7, 8, 9], 2), "1.2");
        assert_eq!(quick_round(vec![1, 2, 3, 4, 5, 6, 7, 8, 9], 3), "1.23");
        assert_eq!(quick_round(vec![1, 2, 3, 4, 5, 6, 7, 8, 9], 4), "1.235");
        assert_eq!(quick_round(vec![1, 2, 3, 4, 5, 6, 7, 8, 9], 5), "1.2346");
        assert_eq!(quick_round(vec![1, 2, 3, 4, 5, 6, 7, 8, 9], 6), "1.23457");
        assert_eq!(quick_round(vec![1, 2, 3, 4, 5, 6, 7, 8, 9], 7), "1.234568");
        assert_eq!(quick_round(vec![1, 2, 3, 4, 5, 6, 7, 8, 9], 8), "1.2345679");
        assert_eq!(quick_round(vec![1, 2, 3, 4, 5, 6, 7, 8, 9], 9), "1.23456789");
    }
}
