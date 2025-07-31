use crate::Number;
use crate::operations::helper_functions::{
    ScientificNumber, float_to_exact_rational, power_rational_and_rational,
};
use astro_float::ctx::Context;
use astro_float::{BigFloat, Consts, RoundingMode, expr};
use num_rational::BigRational;
use num_traits::{Signed, ToPrimitive, Zero};
use regex::Regex;
use std::cmp::Ordering;

pub fn create_default_context() -> Context {
    Context::new(
        1024,
        RoundingMode::ToEven,
        Consts::new().expect("Constants cache initialized"),
        -100000,
        100000,
    )
}

macro_rules! inexact_if_needed {
    ($expr:expr, $item:ident) => {{
        let mut result = $expr;
        if $item.inexact() {
            result.set_inexact(true);
        }
        result
    }};
    ($expr:expr, $item:ident, $item2:ident) => {{
        let mut result = $expr;
        if $item.inexact() || $item2.inexact() {
            result.set_inexact(true);
        }
        result
    }};
}

mod helper_functions {
    use crate::Number;
    use astro_float::ctx::Context;
    use astro_float::{BigFloat, Radix, Word, expr};
    use num_bigint::BigInt;
    use num_rational::BigRational;
    use num_traits::{One, ToPrimitive, Zero};
    use std::cmp::Ordering;
    use std::str::FromStr;

    pub(super) fn power_rational_and_rational(
        base: &BigRational, exponent: &BigRational, ctx: &mut Context,
    ) -> Number {
        macro_rules! safe_return_float_calculation {
            () => {
                let base_float = float_from_rational(base, ctx);
                let exponent_float = float_from_rational(exponent, ctx);
                return Number::from(inexact_if_needed!(
                    expr!(pow(base_float, exponent_float), &mut *ctx),
                    base_float,
                    exponent_float
                ));
            };
        }
        if exponent.is_one() {
            // x1/x2 ^ 1 = x1/x2
            return Number::from(base.clone());
        }
        if exponent.is_zero() {
            // x1/x2 ^ 0 = 1
            return Number::from(BigRational::one()); // any number to the power of 0 is 1
        }
        if exponent.is_integer() {
            // x1/x2 ^ n = (x1^n)/(x2^n)
            let Some(exp) = exponent.to_i32() else {
                safe_return_float_calculation!();
            };
            return Number::from(base.pow(exp));
        }

        // If we reach here, we have a case like x1/x2 ^ (n/d)
        // We now do the following: (x1/x2 ^ n) ^ (1/d)
        let intermediate = if exponent.numer().is_one() {
            base.clone()
        } else {
            let Some(exp) = exponent.numer().to_i32() else {
                safe_return_float_calculation!();
            };
            base.pow(exp)
        };

        // Now the exponents' numerator is handled, we need to handle the denominator

        let numer_result = big_int_to_power_of_inv_of_big_int(intermediate.numer(), exponent.denom(), ctx);
        let denom_result = big_int_to_power_of_inv_of_big_int(intermediate.denom(), exponent.denom(), ctx);
        numer_result.div(&denom_result, ctx)
    }

    /// Calculates the result of a^(1/b).
    /// If a precise result is not possible, it returns a float.
    pub(super) fn big_int_to_power_of_inv_of_big_int(a: &BigInt, b: &BigInt, ctx: &mut Context) -> Number {
        let float_a = BigFloat::from_str(&a.to_string()).unwrap();
        let float_b = BigFloat::from_str(&b.to_string()).unwrap();
        let result = expr!(pow(float_a, 1 / float_b), &mut *ctx);

        if ctx.precision() < 164 {
            return Number::from(result);
        } // if precision is too low rounding is not possible

        let rounding_precision = (ctx.precision() - 100).min((ctx.precision() * 8) / 10);

        let result_rounded = result.round(rounding_precision, ctx.rounding_mode());

        // unwrap is safe here because b is neither nan nor infinity
        let result_rational = rational_from_float(&result_rounded).unwrap();
        let Some(b_i32) = b.to_i32() else {
            return if expr!(pow(result_rounded, float_b), &mut *ctx) == float_a {
                let mut made_exact = result_rounded;
                made_exact.set_inexact(false);
                Number::from(made_exact)
            } else {
                Number::from(result)
            };
        };
        let converted_back = result_rational.pow(b_i32);
        if converted_back.is_integer() && converted_back.numer() == a {
            Number::from(result_rational)
        } else {
            Number::from(result)
        }
    }

    /// Converts a `BigFloat` to a `BigRational` if it is exact.
    /// Returns `None` if the float is inexact or cannot be converted.
    pub fn float_to_exact_rational(float: &BigFloat) -> Option<BigRational> {
        if float.inexact() {
            return None;
        }
        rational_from_float(float)
    }

    pub(super) fn rational_from_string(s: &str) -> Option<BigRational> {
        BigRational::from_str(s).ok()
    }

    pub(super) fn float_from_rational(rational: &BigRational, ctx: &mut Context) -> BigFloat {
        let a_float = BigFloat::from_str(&rational.numer().to_string()).unwrap();
        let b_float = BigFloat::from_str(&rational.denom().to_string()).unwrap();
        expr!(a_float / b_float, &mut *ctx)
    }

    /// Returns `None` if the float is inf or NaN.
    pub(super) fn rational_from_float(float: &BigFloat) -> Option<BigRational> {
        use num_bigint::Sign as IntSign;
        let sign_positive = float.sign()?.is_positive();
        let exponent = float.exponent()?;
        let mantissa = float.mantissa_digits()?;
        let bytes: Vec<u8> = mantissa.iter().rev().map(|v| v.to_be_bytes().into_iter()).flatten().collect();
        let numerator = BigRational::from_integer(BigInt::from_bytes_be(
            if sign_positive { IntSign::Plus } else { IntSign::Minus },
            &bytes,
        ));
        let exp_adj = exponent - (mantissa.len() * size_of::<Word>() * 8) as i32;
        let ratio = numerator * BigRational::from_integer(2.into()).pow(exp_adj);
        Some(ratio)
    }

    #[derive(Debug)]
    pub(super) struct ScientificNumber {
        negative: bool,
        base: Vec<u8>,
        exponent: i64,
    }

    impl ScientificNumber {
        pub(super) const DEFAULT_NON_SCIENTIFIC_DECIMALS: usize = 12;

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

        pub(super) fn to_string(&self, round_to_decimals: usize, non_scientific_decimals: usize) -> String {
            let round_to_decimals = round_to_decimals.max(1);
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
            if exponent.abs() as usize > non_scientific_decimals {
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
}

impl From<BigRational> for Number {
    fn from(value: BigRational) -> Self {
        Self::Rational(value)
    }
}

impl From<BigFloat> for Number {
    fn from(value: BigFloat) -> Self {
        if let Some(rational) = float_to_exact_rational(&value) {
            Self::Rational(rational)
        } else {
            Self::Float(value)
        }
    }
}

impl From<usize> for Number {
    fn from(value: usize) -> Self {
        Self::Rational(BigRational::from_integer(value.into()))
    }
}

impl From<i32> for Number {
    fn from(value: i32) -> Self {
        Self::Rational(BigRational::from_integer(value.into()))
    }
}

// implement external interaction with the Number type
impl Number {
    pub const DEFAULT_ROUNDING_DIGITS: usize = 20;

    pub fn from_string(str: impl ToString) -> Option<Self> {
        let string = str.to_string();
        // remove "_" in between digits like "1_000" to "1000"
        let string = Regex::new(r"(\d)_(\d)").unwrap().replace_all(&string, "$1$2");
        if string.chars().any(|c| !matches!(c, '0'..='9' | '.' | '-')) {
            return None; // only digits and dot are allowed
        }
        if string.chars().filter(|c| *c == '.').count() > 1 {
            return None; // only one dot is allowed
        }
        if let Some(dot_index) = string.find('.').map(|i| string.len() - 1 - i) {
            let just_numbers = string.replace(".", "");
            let rational = helper_functions::rational_from_string(&just_numbers)?;
            let factor = BigRational::from_integer(10.into()).pow(dot_index as i32);
            Some(Self::Rational(rational / factor))
        } else {
            Some(Self::Rational(helper_functions::rational_from_string(&string)?))
        }
    }

    pub fn is_exact(&self) -> bool {
        match self {
            Number::Rational(_) => true,
            Number::Float(float) => !float.inexact(),
        }
    }

    pub fn get_exact_rational(&self) -> Option<BigRational> {
        match self {
            Number::Rational(r) => Some(r.clone()),
            Number::Float(f) => float_to_exact_rational(f),
        }
    }

    pub(crate) fn get_float(&self, ctx: &mut Context) -> BigFloat {
        match self {
            Number::Rational(r) => helper_functions::float_from_rational(r, ctx),
            Number::Float(f) => f.clone(),
        }
    }

    pub fn to_string(&self, rounding_digits: usize, ctx: &mut Context) -> String {
        let float = self.get_float(ctx);
        if let Some(scientific) = ScientificNumber::from_big_float(&float, ctx) {
            scientific.to_string(rounding_digits, ScientificNumber::DEFAULT_NON_SCIENTIFIC_DECIMALS)
        } else {
            float.to_string()
        }
    }

    pub fn get_debug_string(&self) -> String {
        match self {
            Number::Rational(r) => format!("rat({})", r),
            Number::Float(f) => format!("flt({})", f),
        }
    }

    // This is not marked as inexact
    pub fn nan() -> Self {
        Self::Float(BigFloat::nan(None))
    }

    pub fn is_nan(&self) -> bool {
        match self {
            Number::Rational(_) => false,
            Number::Float(f) => f.is_nan(),
        }
    }

    /// Returns `None` when one of the arguments is NaN
    pub fn cmp(&self, other: &Self, ctx: &mut Context) -> Option<Ordering> {
        if let (Some(a), Some(b)) = (self.get_exact_rational(), other.get_exact_rational()) {
            Some(a.cmp(&b))
        } else {
            self.get_float(ctx).partial_cmp(&other.get_float(ctx))
        }
    }
}

// implement operations where there is a list of numbers as a parameter
impl Number {
    pub fn sum(numbers: &[Self], ctx: &mut Context) -> Self {
        numbers.iter().fold(Number::from(0), |a, b| a.plus(&b, ctx))
    }

    pub fn average(numbers: &[Self], ctx: &mut Context) -> Option<Self> {
        match numbers.len() {
            0 => return None,
            1 => return Some(numbers[0].clone()),
            _ => {},
        }
        Some(Self::sum(numbers, ctx).div(&Number::from(numbers.len()), ctx))
    }

    /// Returns `None` if numbers is an empty array
    pub fn median(numbers: &[Self], ctx: &mut Context) -> Option<Self> {
        let num_args = numbers.len();
        match num_args {
            0 => return None,
            1 => return Some(numbers[0].clone()),
            _ => {},
        }
        if numbers.iter().any(|num| num.is_nan()) {
            return Some(Self::nan());
        }
        let mut sorted = numbers.to_vec();
        sorted.sort_by(|a, b| a.cmp(b, ctx).unwrap());
        let middle = num_args / 2;
        Some(if num_args % 2 == 1 {
            sorted[middle].clone()
        } else {
            sorted[middle - 1].plus(&sorted[middle], ctx).div(&Number::from_string("2").unwrap(), ctx)
        })
    }

    pub fn max_of_several(numbers: &[Self], ctx: &mut Context) -> Option<Self> {
        let num_args = numbers.len();
        match num_args {
            0 => return None,
            1 => return Some(numbers[0].clone()),
            _ => {},
        }
        if numbers.iter().any(|num| num.is_nan()) {
            return Some(Self::nan());
        }
        Some(numbers.iter().max_by(|a, b| a.cmp(b, ctx).unwrap()).unwrap().clone())
    }
    pub fn min_of_several(numbers: &[Self], ctx: &mut Context) -> Option<Self> {
        let num_args = numbers.len();
        match num_args {
            0 => return None,
            1 => return Some(numbers[0].clone()),
            _ => {},
        }
        if numbers.iter().any(|num| num.is_nan()) {
            return Some(Self::nan());
        }
        Some(numbers.iter().min_by(|a, b| a.cmp(b, ctx).unwrap()).unwrap().clone())
    }
}

// implement operations where there are two numbers as parameters
impl Number {
    pub fn plus(&self, other: &Self, ctx: &mut Context) -> Self {
        if let (Some(a), Some(b)) = (self.get_exact_rational(), other.get_exact_rational()) {
            Self::from(a + b)
        } else {
            let (a, b) = (self.get_float(ctx), other.get_float(ctx));
            Self::from(inexact_if_needed!(expr!(a + b, &mut *ctx), a, b))
        }
    }

    pub fn mul(&self, other: &Self, ctx: &mut Context) -> Self {
        if let (Some(a), Some(b)) = (self.get_exact_rational(), other.get_exact_rational()) {
            Self::from(a * b)
        } else {
            let (a, b) = (self.get_float(ctx), other.get_float(ctx));
            Self::from(inexact_if_needed!(expr!(a * b, &mut *ctx), a, b))
        }
    }

    pub fn div(&self, other: &Self, ctx: &mut Context) -> Self {
        if let (Some(a), Some(b)) = (self.get_exact_rational(), other.get_exact_rational()) {
            if b.is_zero() {
                return Self::nan(); // handle division by zero
            }
            return Self::from(a / b);
        }
        let (a, b) = (self.get_float(ctx), other.get_float(ctx));
        if b.is_zero() {
            return Self::nan(); // handle division by zero
        }
        Self::from(inexact_if_needed!(expr!(a / b, &mut *ctx), a, b))
    }

    pub fn pow(&self, other: &Self, ctx: &mut Context) -> Self {
        if let (Some(a), Some(b)) = (self.get_exact_rational(), other.get_exact_rational()) {
            return power_rational_and_rational(&a, &b, ctx);
        }
        if let (Some(a), Some(b)) = (
            self.get_exact_rational(),
            other.get_exact_rational().and_then(|r| {
                if !r.is_integer() {
                    return None;
                }
                r.to_i32()
            }),
        ) {
            return Self::from(a.pow(b));
        }

        let (a, b) = (self.get_float(ctx), other.get_float(ctx));
        Self::from(inexact_if_needed!(expr!(pow(a, b), &mut *ctx), a, b))
    }

    pub fn max(&self, other: &Self, ctx: &mut Context) -> Self {
        let Some(comp) = self.cmp(other, ctx) else { return Self::nan() };
        if comp.is_ge() { self.clone() } else { other.clone() }
    }

    pub fn min(&self, other: &Self, ctx: &mut Context) -> Self {
        let Some(comp) = self.cmp(other, ctx) else { return Self::nan() };
        if comp.is_le() { self.clone() } else { other.clone() }
    }
}

// implement operations for just one number
impl Number {
    pub fn neg(&self, _ctx: &mut Context) -> Self {
        match self {
            Self::Rational(r) => Self::from(-r),
            Self::Float(f) => Self::from(inexact_if_needed!(f.neg(), f)),
        }
    }

    pub fn abs(&self, ctx: &mut Context) -> Self {
        if let Some(r) = self.get_exact_rational() {
            Self::from(r.abs())
        } else {
            let float = self.get_float(ctx);
            Self::from(inexact_if_needed!(float.abs(), float))
        }
    }

    pub fn floor(&self, ctx: &mut Context) -> Self {
        if let Some(r) = self.get_exact_rational() {
            Self::from(r.floor())
        } else {
            let float = self.get_float(ctx);
            Self::from(inexact_if_needed!(float.floor(), float))
        }
    }

    pub fn ceil(&self, ctx: &mut Context) -> Self {
        if let Some(r) = self.get_exact_rational() {
            Self::from(r.ceil())
        } else {
            let float = self.get_float(ctx);
            Self::from(inexact_if_needed!(float.ceil(), float))
        }
    }
    pub fn round(&self, ctx: &mut Context) -> Self {
        if let Some(r) = self.get_exact_rational() {
            Self::from(r.round())
        } else {
            let float = self.get_float(ctx);
            Self::from(inexact_if_needed!(float.round(ctx.precision(), ctx.rounding_mode()), float))
        }
    }
}

/// Macro to implement operations that convert the number to a float and apply the operation
macro_rules! float_op {
    ($op:ident) => {
        pub fn $op(&self, ctx: &mut Context) -> Self {
            let float = self.get_float(ctx);
            Number::from(inexact_if_needed!(expr!($op(float), &mut *ctx), float))
        }
    };
}

// implement operations where num needs to be converted to a float
impl Number {
    float_op!(sin);
    float_op!(asin);
    float_op!(cos);
    float_op!(acos);
    float_op!(tan);
    float_op!(atan);

    float_op!(sqrt); // todo check if this can be implemented with rational numbers
    float_op!(log2); // todo check if this can be implemented with rational numbers
    float_op!(log10);
    float_op!(ln);
}

#[cfg(test)]
mod tests {
    use crate::Number;
    use crate::operations::create_default_context;
    use crate::operations::helper_functions::{
        ScientificNumber, big_int_to_power_of_inv_of_big_int, power_rational_and_rational,
        rational_from_float,
    };
    use astro_float::BigFloat;
    use num_bigint::BigInt;
    use num_rational::BigRational;
    use std::str::FromStr;

    fn creat_rational(a: impl Into<BigInt>, b: impl Into<BigInt>) -> BigRational {
        BigRational::new(a.into(), b.into())
    }

    #[test]
    fn test_big_int_to_power_of_inv_of_big_int() {
        let mut ctx = create_default_context();
        let values = [
            (4, 2, creat_rational(2, 1)),
            (9, 2, creat_rational(3, 1)),
            (16, 2, creat_rational(4, 1)),
            (27, 3, creat_rational(3, 1)),
            (64, 3, creat_rational(4, 1)),
            (1000, 3, creat_rational(10, 1)),
            (1000000, 6, creat_rational(10, 1)),
            (1024, 10, creat_rational(2, 1)), // 1024^(1/10) = 2
        ];
        for (a, b, expected) in values {
            println!("Testing: {} ^ (1/{})", a, b);
            let result = big_int_to_power_of_inv_of_big_int(&a.into(), &b.into(), &mut ctx);
            assert_eq!(result.get_exact_rational(), Some(expected));
        }
    }

    #[test]
    pub fn test_power_rational_and_rational() {
        let values = [
            (creat_rational(16, 1), creat_rational(2, 1), creat_rational(256, 1)),
            (creat_rational(16, 1), creat_rational(1, 2), creat_rational(4, 1)),
            (creat_rational(4, 1), creat_rational(1, 2), creat_rational(2, 1)),
            (creat_rational(25, 9), creat_rational(1, 2), creat_rational(5, 3)),
        ];
        let mut ctx = create_default_context();
        for (base, exponent, expected) in values {
            println!("Testing: {} ^ {}", base, exponent);
            let result = power_rational_and_rational(&base, &exponent, &mut ctx);
            assert_eq!(result.get_exact_rational(), Some(expected));
        }
    }

    #[test]
    fn test_create_number_from_string() {
        let mut ctx = create_default_context();

        let mut assert_output_eq = |a: &str, b: Option<(Number, &str)>| {
            println!("Testing: {} == {:?}", a, b);
            println!("Testing conversion to Number");
            let number = Number::from_string(a);
            assert_eq!(number.as_ref(), b.as_ref().map(|s| &s.0));
            println!("Testing conversion to string");
            assert_eq!(
                number.map(|n| n.to_string(Number::DEFAULT_ROUNDING_DIGITS, &mut ctx)),
                b.as_ref().map(|s| s.1.to_string())
            );
        };

        assert_output_eq("1", Some((Number::Rational(creat_rational(1, 1)), "1")));
        assert_output_eq("1.23", Some((Number::Rational(creat_rational(123, 100)), "1.23")));
        assert_output_eq("-1.23", Some((Number::Rational(creat_rational(-123, 100)), "-1.23")));
        assert_output_eq("1000", Some((Number::Rational(creat_rational(1000, 1)), "1000")));
        assert_output_eq("1_000", Some((Number::Rational(creat_rational(1000, 1)), "1000")));
        assert_output_eq("1000_", None);
        assert_output_eq("_1000", None);
        assert_output_eq("1_000_000", Some((Number::Rational(creat_rational(1000000, 1)), "1000000")));
    }

    #[test]
    fn test_convert_float_to_rational() {
        let inputs_and_expected = [
            ("3.5", Some(creat_rational(7, 2))),
            (
                &format!("1{}21", "0".repeat(100)),
                BigRational::from_str(&format!("1{}21", "0".repeat(100))).unwrap().into(),
            ),
            ("1.125", Some(creat_rational(9, 8))),
            ("inf", None),
            ("-inf", None),
        ];
        for (i, o) in inputs_and_expected {
            let float = BigFloat::from_str(i).unwrap();
            assert_eq!(rational_from_float(&float), o);
        }
    }

    fn num(str: impl ToString) -> Number {
        Number::from_string(str).unwrap()
    }

    #[test]
    fn test_calculation_result_is_exact() {
        let ctx = &mut create_default_context();
        macro_rules! exact_check {
            ($calc:expr, $expected:expr) => {
                println!("Test case {} == {}", stringify!($calc), $expected);
                assert_eq!($calc.is_exact(), $expected);
            };
        }
        exact_check!(num(1), true);
        exact_check!(num(1).sin(ctx), false);
        exact_check!(num(1).sin(ctx).plus(&num(0), ctx), false);
        exact_check!(num(1).sin(ctx).plus(&num(0), ctx).mul(&num(2), ctx), false);
        exact_check!(num(4).sqrt(ctx), true);
        exact_check!(num(4).pow(&num("0.5"), ctx), true);
    }
    // `find_min!` will calculate the minimum of any number of arguments.

    #[test]
    fn test_calculation_string_output() {
        let ctx = &mut create_default_context();

        macro_rules! short_assert_eq {
            // Base case:
            (($calc:expr, $expected:expr, $rounding:expr)) => (
                println!("Checking: {} == {} with rounding {}", stringify!($calc), $expected, $rounding);
                assert_eq!($calc.to_string($rounding, ctx), $expected)
            );
            // `$x` followed by at least one `$y,`
            (($calc:expr, $expected:expr, $rounding:expr), $(($a:expr, $b:expr, $c:expr)), + ) => (
                short_assert_eq!(($calc, $expected, $rounding));
                short_assert_eq!($(($a, $b, $c)),+)
            )
        }

        short_assert_eq!(
            (num("1"), "1", 20),
            (num("-1"), "-1", 20),
            (num("1.2"), "1.2", 20),
            (num("-1.2"), "-1.2", 20),
            (num("1.23"), "1.23", 20),
            (num("1.234"), "1.234", 20),
            (num("0.2").plus(&num("0.1"), ctx), "0.3", 20),
            (num("-0.2").plus(&num("-0.1"), ctx), "-0.3", 20),
            (num("0.2").plus(&num("0.1"), ctx).div(&num("3"), ctx), "0.1", 20),
            (num("-0.2").plus(&num("-0.1"), ctx).div(&num("3"), ctx), "-0.1", 20),
            (num("1").asin(ctx).mul(&num("2"), ctx), "3.1416", 5),
            (num("1").asin(ctx).mul(&num("2"), ctx), "3.14159", 6),
            (num("1").asin(ctx).mul(&num("2"), ctx), "3.14159265", 9),
            (num("1").asin(ctx).sin(ctx), "1", 10),
            (num("1").asin(ctx).sin(ctx), "1", 30),
            (num("2").div(&num("3"), ctx), "0.6666666666666666666666666667", 28),
            (num("2").div(&num("3"), ctx), "0.66666666666666666666666666667", 29)
        );
    }

    #[test]
    fn test_scientific_number() {
        fn quick_conversion(base: Vec<u8>, exponent: i64, no_sci_digits: usize) -> String {
            ScientificNumber::new(false, base, exponent).to_string(100, no_sci_digits)
        }
        assert_eq!(quick_conversion(vec![0, 0, 0], 0, 100), "0");
        assert_eq!(quick_conversion(vec![0, 0, 0], 3, 100), "0");
        assert_eq!(quick_conversion(vec![1], -1, 100), "0.1");
        assert_eq!(quick_conversion(vec![1], 0, 100), "1");
        assert_eq!(quick_conversion(vec![1], 3, 100), "1000");

        assert_eq!(quick_conversion(vec![1, 2, 3], -2, 100), "0.0123");
        assert_eq!(quick_conversion(vec![1, 2, 3], -1, 100), "0.123");
        assert_eq!(quick_conversion(vec![1, 2, 3], 0, 100), "1.23");
        assert_eq!(quick_conversion(vec![1, 2, 3], 1, 100), "12.3");
        assert_eq!(quick_conversion(vec![1, 2, 3], 2, 100), "123");
        assert_eq!(quick_conversion(vec![1, 2, 3], 3, 100), "1230");
        assert_eq!(quick_conversion(vec![1, 2, 3], 4, 100), "12300");

        assert_eq!(quick_conversion(vec![1, 2, 3], -2, 0), "1.23e-2");
        assert_eq!(quick_conversion(vec![1, 2, 3], -1, 0), "1.23e-1");
        assert_eq!(quick_conversion(vec![1, 2, 3], 0, 0), "1.23");
        assert_eq!(quick_conversion(vec![1, 2, 3], 1, 0), "1.23e1");
        assert_eq!(quick_conversion(vec![1, 2, 3], 2, 0), "1.23e2");

        assert_eq!(quick_conversion(vec![1, 2, 3], -2, 1), "1.23e-2");
        assert_eq!(quick_conversion(vec![1, 2, 3], -1, 1), "0.123");
        assert_eq!(quick_conversion(vec![1, 2, 3], 0, 1), "1.23");
        assert_eq!(quick_conversion(vec![1, 2, 3], 1, 1), "12.3");
        assert_eq!(quick_conversion(vec![1, 2, 3], 2, 1), "1.23e2");
        fn quick_round(base: Vec<u8>, round_to_decimals: usize) -> String {
            ScientificNumber::new(false, base, 0)
                .to_string(round_to_decimals, ScientificNumber::DEFAULT_NON_SCIENTIFIC_DECIMALS)
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
