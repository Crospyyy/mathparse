use crate::operations::helper_functions::{float_to_exact_rational, power_rational_and_rational};
use crate::printing::{FormattingOptions, ScientificNumber};
use crate::{Element, Number, NumberContext};
use astro_float::ctx::Context;
use astro_float::{BigFloat, Consts, Error, RoundingMode, expr};
use num_rational::BigRational;
use num_traits::{Signed, ToPrimitive, Zero};
use regex::Regex;
use std::cmp::Ordering;
use std::ops::Neg;
use std::sync::LazyLock;

pub fn create_default_context() -> NumberContext {
    NumberContext::new(
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
        if $item.is_nan() {
            $item.clone()
        } else if $item2.is_nan() {
            $item2.clone()
        } else {
            let mut result = $expr;
            if $item.inexact() || $item2.inexact() {
                result.set_inexact(true);
            }
            result
        }
    }};
}

mod helper_functions {
    use crate::Number;
    use crate::operations::rational;
    use astro_float::ctx::Context;
    use astro_float::{BigFloat, Error, Word, expr};
    use num_bigint::BigInt;
    use num_rational::BigRational;
    use num_traits::{One, Signed, ToPrimitive, Zero};
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
        if exponent.is_negative() && base.is_zero() {
            return Number::nan(Some(Error::DivisionByZero));
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
        let result = if b == &2.into() {
            expr!(sqrt(float_a), &mut *ctx)
        } else {
            expr!(pow(float_a, 1 / float_b), &mut *ctx)
        };

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
        if *float == BigFloat::from(1) {
            return Some(rational(1));
        }
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
}
pub(crate) fn rational(n: i32) -> BigRational {
    BigRational::from_integer(n.into())
}

/// r = radians (0..1 = top right, 1..2 = top left, 2..3 = bottom left, 3..4 = bottom right)
pub(crate) fn sin_radians(r: &BigRational, ctx: &mut Context) -> Number {
    dbg!(&r);
    let mut r = r.clone() % rational(4);
    if r.is_negative() {
        r += rational(4);
    }
    dbg!(&r);
    let mut mapped = if r < rational(1) {
        r
    } else if r < rational(2) {
        rational(2) - r
    } else if r < rational(3) {
        rational(2) - r
    } else {
        r - rational(4)
    };

    fn negate_if(num: i32, neg: bool) -> i32 {
        if neg { -num } else { num }
    }
    if mapped == rational(0) {
        dbg!("returning zero");
        return Number::from(0);
    }
    if mapped.abs() == rational(1) {
        dbg!("returning one");
        return Number::from(negate_if(1, mapped.is_negative()));
    }
    if mapped.abs() == BigRational::new(1.into(), 3.into()) {
        dbg!("returning 1/2");
        return Number::Rational(BigRational::new(negate_if(1, mapped.is_negative()).into(), 2.into()));
    }
    if mapped.abs() == BigRational::new(2.into(), 3.into()) {
        dbg!("returning 1/2");
        let negator = negate_if(1, mapped.is_negative());
        return Number::from(expr!(negator * sqrt(3) / 2, &mut *ctx));
    }

    dbg!("return sin calculation");
    let float = helper_functions::float_from_rational(&(mapped / rational(2)), ctx);
    let sin_input = expr!(float * pi, &mut *ctx);
    let float = expr!(sin(sin_input), &mut *ctx);
    Number::from(float)
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

impl PartialEq for Number {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Number::Rational(a), Number::Rational(b)) => a == b,
            (Number::Float(a), Number::Float(b)) => {
                a.eq(b) || (a.is_nan() && b.is_nan() && (a.err() == b.err()))
            },
            _ => false,
        }
    }
}

impl PartialEq<i32> for Number {
    fn eq(&self, other: &i32) -> bool {
        match self {
            Number::Rational(r) => r.is_integer() && r.to_i32() == Some(*other),
            Number::Float(f) => f.is_int() && f == &BigFloat::from(*other),
        }
    }
}

impl PartialEq<i32> for &Number {
    fn eq(&self, other: &i32) -> bool {
        match self {
            Number::Rational(r) => r.is_integer() && r.to_i32() == Some(*other),
            Number::Float(f) => f.is_int() && f == &BigFloat::from(*other),
        }
    }
}

impl PartialEq<i32> for &Element {
    fn eq(&self, other: &i32) -> bool {
        self.get_number_inner().is_some_and(|n| n == other)
    }
}

impl Element {
    pub(crate) fn is(&self, other: i32) -> bool {
        self.get_number_inner().is_some_and(|n| *n == other)
    }

    pub(crate) fn is_neg(&self, other: i32) -> bool {
        self.get_negate_inner().is_some_and(|e| e.is(other))
    }

    pub(crate) fn is_nan(&self) -> bool {
        self.get_number_inner().is_some_and(|n| n.is_nan())
    }
}

impl Number {
    pub(crate) fn is_negative(&self) -> bool {
        match self {
            Number::Rational(r) => r.is_negative(),
            Number::Float(f) => f.is_negative(),
        }
    }
    pub(crate) fn is_integer(&self) -> bool {
        match self {
            Number::Rational(r) => r.is_integer(),
            Number::Float(f) => f.is_int(),
        }
    }
}

static REGEX_NUMBER_UNDERSCORE_REMOVAL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d)_(\d)").unwrap());

// implement external interaction with the Number type
impl Number {
    pub fn from_string(str: impl ToString) -> Option<Self> {
        let string = str.to_string();
        // remove "_" in between digits like "1_000" to "1000"
        let string = REGEX_NUMBER_UNDERSCORE_REMOVAL.replace_all(&string, "$1$2");
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

    pub fn to_string(&self, formatting_options: FormattingOptions, ctx: &mut Context) -> String {
        let float = self.get_float(ctx);
        if let Some(scientific) = ScientificNumber::from_big_float(&float, ctx) {
            scientific.to_string(formatting_options)
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
    pub fn nan(error: Option<Error>) -> Self {
        Self::Float(BigFloat::nan(error))
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
            return Some(Self::nan(None));
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
            return Some(Self::nan(None));
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
            return Some(Self::nan(None));
        }
        Some(numbers.iter().min_by(|a, b| a.cmp(b, ctx).unwrap()).unwrap().clone())
    }
}

// implement operations where there are two numbers as parameters
impl Number {
    pub fn rem(&self, other: &Self, ctx: &mut Context) -> Self {
        if let (Number::Rational(r1), Number::Rational(r2)) = (self, other) {
            if r2.is_zero() {
                return Self::nan(None);
            }
            let mut result = r1 % r2;
            if result.is_negative() {
                result += r2;
            }
            Self::from(result)
        } else {
            let self_float = self.get_float(ctx);
            let other_float = other.get_float(ctx);
            Self::from(inexact_if_needed!(
                expr!(self_float % other_float, &mut *ctx),
                self_float,
                other_float
            ))
        }
    }

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
            if self.is_nan() {
                return self.clone();
            } else if other.is_nan() {
                return other.clone();
            }
            let (a, b) = (self.get_float(ctx), other.get_float(ctx));
            Self::from(inexact_if_needed!(expr!(a * b, &mut *ctx), a, b))
        }
    }

    pub fn div(&self, other: &Self, ctx: &mut Context) -> Self {
        if let (Some(a), Some(b)) = (self.get_exact_rational(), other.get_exact_rational()) {
            if b.is_zero() {
                return Self::nan(None); // handle division by zero
            }
            return Self::from(a / b);
        }
        let (a, b) = (self.get_float(ctx), other.get_float(ctx));
        if b.is_zero() {
            return Self::nan(None); // handle division by zero
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
        let Some(comp) = self.cmp(other, ctx) else { return Self::nan(None) };
        if comp.is_ge() { self.clone() } else { other.clone() }
    }

    pub fn min(&self, other: &Self, ctx: &mut Context) -> Self {
        let Some(comp) = self.cmp(other, ctx) else { return Self::nan(None) };
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
            let ceil = float.ceil();
            let diff_ceil = float.sub(&ceil, ctx.precision(), ctx.rounding_mode()).abs();

            match diff_ceil.partial_cmp(&BigFloat::from(0.5)) {
                None => {
                    Self::nan(None) // cannot compare, return NaN
                },
                Some(Ordering::Less) | Some(Ordering::Equal) => Self::from(inexact_if_needed!(ceil, float)),
                Some(Ordering::Greater) => Self::from(inexact_if_needed!(float.floor(), float)),
            }
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

    float_op!(log2); // todo check if this can be implemented with rational numbers
    float_op!(log10);
    float_op!(ln);
}

pub(crate) mod expression_functions {
    use crate::Number;
    use crate::expression_values::{ExpressionFunType, ExpressionNumType};
    use crate::parsing::signature::ParamCount;
    use astro_float::ctx::Context;

    fn pi(ctx: &mut Context) -> Number {
        Number::Float(ctx.const_pi())
    }

    fn e(ctx: &mut Context) -> Number {
        Number::Float(ctx.const_e())
    }
    impl ExpressionNumType {
        pub(crate) fn get_function(&self) -> fn(&mut Context) -> Number {
            match self {
                ExpressionNumType::Pi => pi,
                ExpressionNumType::E => e,
            }
        }
    }
    impl ExpressionFunType {
        pub(crate) fn get_function_single_arg(&self) -> Option<fn(&Number, &mut Context) -> Number> {
            match self {
                ExpressionFunType::Sin => Number::sin,
                ExpressionFunType::Asin => Number::asin,
                ExpressionFunType::Cos => Number::cos,
                ExpressionFunType::Acos => Number::acos,
                ExpressionFunType::Tan => Number::tan,
                ExpressionFunType::Atan => Number::atan,
                ExpressionFunType::Floor => Number::floor,
                ExpressionFunType::Round => Number::round,
                ExpressionFunType::Rem => None?,
                ExpressionFunType::Log2 => Number::log2,
            }
                .into()
        }

        pub(crate) fn get_function_multiple_args(&self) -> Option<fn(&mut Context, Vec<Number>) -> Number> {
            match self {
                ExpressionFunType::Rem => Some(|ctx: &mut Context, args: Vec<Number>| {
                    if args.len() != 2 {
                        eprintln!("rem function expects exactly two arguments, but got {}", args.len());
                        return Number::nan(None);
                    }
                    Number::rem(&args[0], &args[1], ctx)
                }),
                ExpressionFunType::Sin
                | ExpressionFunType::Asin
                | ExpressionFunType::Cos
                | ExpressionFunType::Acos
                | ExpressionFunType::Tan
                | ExpressionFunType::Atan
                | ExpressionFunType::Floor
                | ExpressionFunType::Round
                | ExpressionFunType::Log2 => None?,
            }
        }

        pub(crate) fn get_param_count(&self) -> ParamCount {
            match self {
                ExpressionFunType::Rem => ParamCount::Exactly(2),
                ExpressionFunType::Sin => ParamCount::Exactly(1),
                ExpressionFunType::Asin => ParamCount::Exactly(1),
                ExpressionFunType::Cos => ParamCount::Exactly(1),
                ExpressionFunType::Acos => ParamCount::Exactly(1),
                ExpressionFunType::Tan => ParamCount::Exactly(1),
                ExpressionFunType::Atan => ParamCount::Exactly(1),
                ExpressionFunType::Floor => ParamCount::Exactly(1),
                ExpressionFunType::Round => ParamCount::Exactly(1),
                ExpressionFunType::Log2 => ParamCount::Exactly(1),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Number;
    use crate::operations::helper_functions::{
        big_int_to_power_of_inv_of_big_int, power_rational_and_rational, rational_from_float,
    };
    use crate::operations::{FormattingOptions, create_default_context};
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
                number.map(|n| n.to_string(FormattingOptions::default(), &mut ctx)),
                b.as_ref().map(|s| s.1.to_string())
            );
        };

        assert_output_eq("1", Some((Number::Rational(creat_rational(1, 1)), "1")));
        assert_output_eq("1.23", Some((Number::Rational(creat_rational(123, 100)), "1.23")));
        assert_output_eq("-1.23", Some((Number::Rational(creat_rational(-123, 100)), "-1.23")));
        assert_output_eq("1000", Some((Number::Rational(creat_rational(1000, 1)), "1,000")));
        assert_output_eq("1_000", Some((Number::Rational(creat_rational(1000, 1)), "1,000")));
        assert_output_eq("1000_", None);
        assert_output_eq("_1000", None);
        assert_output_eq("1_000_000", Some((Number::Rational(creat_rational(1000000, 1)), "1,000,000")));
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
                assert_eq!($calc.to_string(FormattingOptions::default().with_rounding($rounding), ctx), $expected)
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
}
