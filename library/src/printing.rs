use crate::calculation::create_context;
use crate::evaluation::DynamicResult;
use crate::expression_values::ExpressionFunType;
use crate::parsing::signature::ParamCount;
use crate::{Element, ExpressionNumType, Number, create_default_context};
use astro_float::ctx::Context;
use astro_float::{BigFloat, Radix};
use colored::Colorize;
use num_bigint::{BigInt, Sign};
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};
use std::cmp::Ordering;
use std::fmt::{Display, Formatter};

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

#[derive(Debug, PartialEq)]
pub(super) struct ScientificNumber {
    negative: bool,
    /// Can be empty, in which case the number is 0
    base: Vec<u8>,
    exponent: i64,
}

/// Options for formatting numbers to strings
/// * `round_to_decimals`: Number of significant digits to round to
/// * `non_scientific_decimals`: Maximum exponent (positive or negative) to print in non-scientific format
/// * `thousands_separator`: Whether to include thousand separators in non-scientific format
/// ### Examples:
/// - 1234567.89 with thousands_separator = true -> "1,234,567.89"
/// - 1.2345 with round_to_decimals = 3 -> "1.23"
/// - 1.2345 with round_to_decimals = 1 or 0 -> "1"
#[derive(Debug, Clone, Copy)]
pub struct FormattingOptions {
    pub round_to_decimals: usize,
    pub non_scientific_decimals: usize,
    pub thousands_separator: bool,
}

pub enum FormattedCalculationOutput {
    Exact { result: String, has_rounded: bool },
    ApproximationChecked { result: String, precision_bits: u32 },
    ApproximationReachedLimit { result: String },
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
            Element::Number(num) => Formula::Number(num.to_string_reuse_context(formatting_options, ctx)),
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
            Element::FunctionWithExpression { arguments, expr_value: debug_name, .. } => Formula::Function {
                name: format!("fun_expr:{debug_name}"),
                arguments: arguments.iter().map(|e| create_formula!(e)).collect(),
            },
            Element::NumberWithExpression { expr_value: debug_name, .. } => {
                Formula::Variable(format!("num_expr:{debug_name}",))
            },
        }
    }
}

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
            Element::FunctionWithExpression { arguments, expr_value } => {
                let param_count_str = match expr_value.get_param_count() {
                    ParamCount::Exactly(n) => format!("={n} params"),
                    ParamCount::AtLeast(n) => format!(">={n} params"),
                };
                format!(
                    "fun_with_expr:{}({}, [{}])",
                    expr_value,
                    param_count_str,
                    arguments.iter().map(Self::get_debug_string).collect::<Vec<_>>().join(", ")
                )
            },
            Element::NumberWithExpression { expr_value: debug_name, .. } => {
                format!("num_with_expr:{}", debug_name)
            },
        }
    }
}

impl Number {
    pub fn to_string_reuse_context(
        &self, formatting_options: FormattingOptions, ctx: &mut Context,
    ) -> String {
        (if let Some(num_rational) = self.get_exact_rational() {
            rational_to_string(&num_rational, formatting_options).0
        } else {
            let float = self.get_float(ctx);
            let Some(scientific) = ScientificNumber::from_big_float(&float, ctx) else {
                return float.to_string();
            };
            scientific.to_string(formatting_options)
        })
            .to_string()
    }

    pub fn to_string(&self, formatting_options: FormattingOptions) -> String {
        self.to_string_reuse_context(formatting_options, &mut create_default_context())
    }
}

impl ScientificNumber {
    /// Creates a new scientific number from its components.
    /// # Arguments
    /// * `negative` - Whether the number is negative
    /// * `base` - The significant digits of the number (e.g., \[1234\] for 1.234)
    /// * `exponent` - The exponent of the number (e.g., 0 for 1.234, 1 for 12.34, -1 for 0.1234)
    pub(super) fn new(negative: bool, base: impl Into<Vec<u8>>, exponent: i64) -> Self {
        let mut base_vec = base.into();
        let mut exponent = exponent;
        while base_vec.first() == Some(&0) {
            base_vec.remove(0);
            exponent -= 1;
        }
        Self { negative, base: base_vec, exponent }
    }

    pub(super) fn from_big_float(float: &BigFloat, ctx: &mut Context) -> Option<Self> {
        let (sign, numbers, exp) =
            float.convert_to_radix(Radix::Dec, ctx.rounding_mode(), ctx.consts()).ok()?;
        Some(Self::new(sign.is_negative(), numbers, exp as i64 - 1))
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
                Some(Self::new(negative, vec![digit as u8], b))
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

        Some(Self::new(negative, numbers, b))
    }

    /// Converts the scientific number to a string with the given formatting options.
    pub(super) fn to_string(&self, options: FormattingOptions) -> String {
        let round_to_decimals = options.round_to_decimals.max(1);
        let mut modified_exponent = self.exponent;
        let (rounded, _has_rounded) =
            ScientificNumber::round_decimals_vec(round_to_decimals, &mut modified_exponent, &self.base);
        if rounded.is_empty() {
            return "0".to_owned();
        }
        if Self::should_print_scientific(&rounded, modified_exponent, options) {
            Self::create_scientific_string(&rounded, modified_exponent, self.negative)
        } else {
            Self::create_regular_string(rounded, modified_exponent, self.negative, options)
        }
    }

    /// Rounds the given vector of digits to the specified number of significant digits and removes leading and trailing zeroes.
    fn round_decimals_vec(round_to_decimals: usize, exponent: &mut i64, vec: &[u8]) -> (Vec<u8>, bool) {
        let mut vec = vec.to_vec();
        // remove trailing zeroes
        while vec.last() == Some(&0) {
            vec.pop();
        }
        // remove leading zeroes
        while vec.first() == Some(&0) {
            vec.remove(0);
            *exponent -= 1;
        }

        let mut has_rounded = true;

        let mut rounded = if round_to_decimals >= vec.len() {
            has_rounded = false;
            vec
        } else {
            has_rounded = true;
            let mut numbers = vec[..=round_to_decimals].to_vec();
            // rounding
            if numbers[round_to_decimals] >= 5 {
                for i in (0..round_to_decimals).rev() {
                    if numbers[i] == 9 {
                        numbers[i] = 0;
                        if i == 0 {
                            numbers.insert(0, 1);
                            *exponent += 1;
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
        // remove trailing zeroes
        while rounded.last() == Some(&0) {
            rounded.pop();
        }
        (rounded, has_rounded)
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

impl DynamicResult {
    pub fn to_string_detailed(&self, formatting_options: FormattingOptions) -> FormattedCalculationOutput {
        match self {
            DynamicResult::Exact(r) => {
                let (string, rounded) = rational_to_string(r, formatting_options);
                FormattedCalculationOutput::Exact { result: string, has_rounded: rounded }
            },
            DynamicResult::Checked { num, precision } => {
                let mut context = create_context(*precision as usize);
                let string = if let Some(scientific) = ScientificNumber::from_big_float(&num, &mut context) {
                    scientific.to_string(formatting_options)
                } else {
                    num.to_string()
                };
                FormattedCalculationOutput::ApproximationChecked {
                    result: string,
                    precision_bits: *precision,
                }
            },
            DynamicResult::ReachedLimit(num) => {
                let mut context = create_context(num.precision().unwrap_or(1));
                let string = if let Some(scientific) = ScientificNumber::from_big_float(&num, &mut context) {
                    scientific.to_string(formatting_options)
                } else {
                    num.to_string()
                };
                FormattedCalculationOutput::ApproximationReachedLimit { result: string }
            },
        }
    }
}

impl FormattedCalculationOutput {
    pub(crate) fn get_string(&self) -> &String {
        match self {
            FormattedCalculationOutput::Exact { result, .. } => result,
            FormattedCalculationOutput::ApproximationChecked { result, .. } => result,
            FormattedCalculationOutput::ApproximationReachedLimit { result } => result,
        }
    }
}

impl Display for Formula {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
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

impl Display for ExpressionFunType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ExpressionFunType::AssertValueRange(r) => f.write_str(&format!("check_{:?}", r).to_lowercase()),
            ExpressionFunType::Custom(c) => f.write_str(&format!("custom_{}", c.name)),
            _ => f.write_str(&camel_to_snake_case(&format!("{:?}", self))),
        }
    }
}

impl Display for ExpressionNumType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ExpressionNumType::Pi => "pi",
            ExpressionNumType::E => "e",
        })
    }
}

impl Default for FormattingOptions {
    fn default() -> Self {
        Self { round_to_decimals: 9, non_scientific_decimals: 12, thousands_separator: true }
    }
}

fn camel_to_snake_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + s.len() / 4);
    for (i, c) in s.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn rational_to_string(
    ratio: &BigRational, formatting_options: FormattingOptions,
) -> (String, bool) {
    let scientific = long_division(&ratio.numer(), &ratio.denom(), formatting_options.round_to_decimals);
    (scientific.0.to_string(formatting_options), scientific.1)
}

/// Performs long division of two BigInts to obtain a ScientificNumber representation of the result.
/// Returns a tuple containing the ScientificNumber and a boolean indicating whether the result was rounded.
fn long_division(
    numerator: &BigInt, denominator: &BigInt, rounding_decimals: usize,
) -> (ScientificNumber, bool) {
    let negative = numerator.is_negative() ^ denominator.is_negative();
    let mut num_vec = numerator.abs().to_radix_be(10).1;
    let mut exponent = (num_vec.len() - 1) as i64;
    let mut has_been_rounded = false;

    let result_vec = if denominator.is_one() {
        num_vec
    } else {
        let denom_abs = denominator.abs();

        let mut leading_zeroes = 0usize;
        let mut result_vec = vec![];
        has_been_rounded = true;
        for take_n_from_numerator in 1.. {
            if result_vec.len() > rounding_decimals {
                break;
            }
            if take_n_from_numerator > num_vec.len() {
                num_vec.push(0);
            }
            let current_num =
                BigInt::from_radix_be(Sign::Plus, &num_vec[leading_zeroes..take_n_from_numerator], 10)
                    .unwrap();
            if current_num.is_zero() && take_n_from_numerator == num_vec.len() {
                has_been_rounded = false;
                break;
            }
            let division_result = &current_num / &denom_abs;
            let result_digit = division_result.to_u8().unwrap();

            if result_digit == 0 {
                if result_vec.is_empty() {
                    exponent -= 1;
                } else {
                    result_vec.push(0);
                }
                continue;
            } else {
                result_vec.push(result_digit);
            }

            let remainder = current_num % &denom_abs;
            if !remainder.is_zero() {
                let rem_digits = remainder.to_radix_be(10).1;
                leading_zeroes = take_n_from_numerator - rem_digits.len();
                num_vec[leading_zeroes..take_n_from_numerator].copy_from_slice(&rem_digits);
            } else {
                leading_zeroes = take_n_from_numerator;
            }
        }
        result_vec
    };

    let (result_rounded, has_rounded) =
        ScientificNumber::round_decimals_vec(rounding_decimals, &mut exponent, &result_vec);
    has_been_rounded |= has_rounded;
    (ScientificNumber::new(negative, result_rounded, exponent), has_been_rounded)
}

#[cfg(test)]
mod tests {
    use crate::FormattingOptions;
    use crate::printing::{ScientificNumber, long_division};
    use num_bigint::BigInt;
    use std::str::FromStr;

    #[test]
    fn test_long_division() {
        fn quick_divide(num: &str, denom: &str, decimals: usize) -> (ScientificNumber, bool) {
            let num = BigInt::from_str(num).unwrap();
            let denom = BigInt::from_str(denom).unwrap();
            long_division(&num, &denom, decimals)
        }

        fn check_divide(
            num: &str, denom: &str, decimals: usize, base: impl Into<Vec<u8>>, exponent: i64, rounded: bool,
            negative: bool,
        ) {
            assert_eq!(
                quick_divide(num, denom, decimals),
                (ScientificNumber::new(negative, base, exponent), rounded)
            );
        }
        check_divide("1", "1", 5, [1], 0, false, false);
        check_divide("3", "1", 5, [3], 0, false, false);
        check_divide("6", "1", 5, [6], 0, false, false);
        check_divide("123456", "1", 5, [1, 2, 3, 4, 6], 5, true, false);

        check_divide("1", "3", 5, [3, 3, 3, 3, 3], -1, true, false);
        check_divide("1", "6", 5, [1, 6, 6, 6, 7], -1, true, false);
        check_divide("1", "9", 5, [1, 1, 1, 1, 1], -1, true, false);

        check_divide("-1", "3", 5, [3, 3, 3, 3, 3], -1, true, true);
        check_divide("-1", "6", 5, [1, 6, 6, 6, 7], -1, true, true);
        check_divide("-1", "9", 5, [1, 1, 1, 1, 1], -1, true, true);

        check_divide("1", "-3", 5, [3, 3, 3, 3, 3], -1, true, true);
        check_divide("1", "-6", 5, [1, 6, 6, 6, 7], -1, true, true);
        check_divide("1", "-9", 5, [1, 1, 1, 1, 1], -1, true, true);

        check_divide("-1", "-3", 5, [3, 3, 3, 3, 3], -1, true, false);
        check_divide("-1", "-6", 5, [1, 6, 6, 6, 7], -1, true, false);
        check_divide("-1", "-9", 5, [1, 1, 1, 1, 1], -1, true, false);

        check_divide("1", "7", 6, [1, 4, 2, 8, 5, 7], -1, true, false);
        check_divide("1", "7", 5, [1, 4, 2, 8, 6], -1, true, false);
        check_divide("1", "7", 4, [1, 4, 2, 9], -1, true, false);
        check_divide("1", "7", 3, [1, 4, 3], -1, true, false);
        check_divide("1", "7", 2, [1, 4], -1, true, false);
        check_divide("1", "7", 1, [1], -1, true, false);

        check_divide("1", "2", 5, [5], -1, false, false);
        check_divide("1", "4", 5, [2, 5], -1, false, false);
        check_divide("1", "8", 5, [1, 2, 5], -1, false, false);
        check_divide("1", "16", 5, [6, 2, 5], -2, false, false);
        check_divide("1", "32", 5, [3, 1, 2, 5], -2, false, false);
        check_divide("1", "64", 5, [1, 5, 6, 2, 5], -2, false, false);
        check_divide("1", "128", 5, [7, 8, 1, 2, 5], -3, false, false);
        check_divide("1", "256", 5, [3, 9, 0, 6, 3], -3, true, false);
    }

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
        assert_eq!(quick_conversion(vec![1, 0, 0], 3, 0), "1e3");
        assert_eq!(quick_conversion(vec![0, 1], 1, 0), "1");
        assert_eq!(quick_conversion(vec![0, 0, 1], 2, 0), "1");

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
