use crate::operations::helper_functions::float_to_exact_rational;
use crate::Number;
use astro_float::ctx::Context;
use astro_float::{expr, BigFloat, Consts, RoundingMode};
use num_rational::BigRational;
use rust_decimal::prelude::{Signed, ToPrimitive, Zero};

pub fn create_default_context() -> Context {
    Context::new(
        1024,
        RoundingMode::ToEven,
        Consts::new().expect("Constants cache initialized"),
        -100000,
        100000,
    )
}

mod helper_functions {
    use astro_float::ctx::Context;
    use astro_float::{expr, BigFloat, Word};
    use num_bigint::BigInt;
    use num_rational::BigRational;
    use rust_decimal::Decimal;
    use std::str::FromStr;

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

    pub(super) fn decimal_from_rational(rational: &BigRational) -> Option<Decimal> {
        let num = Decimal::from_str(&rational.numer().to_string()).ok()?;
        let denom = Decimal::from_str(&rational.denom().to_string()).ok()?;
        Some(num / denom)
    }

    pub(super) fn float_from_rational(rational: &BigRational, ctx: &mut Context) -> BigFloat {
        let a_float = BigFloat::from_str(&rational.numer().to_string()).unwrap();
        let b_float = BigFloat::from_str(&rational.denom().to_string()).unwrap();
        expr!(a_float / b_float, &mut *ctx)
    }

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

    pub(super) fn round_scientific(str: &str, decimals: usize) -> Option<String> {
        let (a, b) = str.split_once("e")?;
        let mut b: i64 = b.parse().ok()?;
        if a.find(".") != Some(1) {
            if a.len() == 1 && a.chars().next().unwrap().is_digit(10) {
                return Some(str.to_string());
            }
            return None;
        }
        let mut numbers: Vec<_> =
            a.replace(".", "").chars().map(|c| c.to_digit(10)).collect::<Option<_>>()?;
        if decimals + 1 > numbers.len() {
            return Some(str.to_owned());
        }
        if numbers[decimals] >= 5 {
            for i in (0..decimals).rev() {
                if numbers[i] == 9 {
                    numbers[i] = 0;
                    if i == 0 {
                        numbers.insert(0, 1);
                        b += 1;
                    }
                } else {
                    numbers[i] += 1;
                    break;
                }
            }
        }

        let mut rounded = numbers[..decimals].to_vec();
        while rounded.len() > 1 && rounded.last() == Some(&0) {
            rounded.pop();
        }
        let mut string = rounded.iter().map(|n| n.to_string()).reduce(|a, b| a + &b).unwrap_or("".to_owned());

        if string.len() > 1 {
            string.insert(1, '.');
        }

        Some(format!("{}e{}", string, b))
    }

    pub(super) fn float_to_string_with_rounding(float: &BigFloat, decimals: usize) -> String {
        let float_string = float.to_string();
        let scientific = round_scientific(&float_string, decimals).unwrap_or(float_string);
        let Ok(d) = Decimal::from_scientific(&scientific) else { return scientific };
        d.to_string()
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
    pub fn from_string(str: impl ToString) -> Option<Self> {
        let string = str.to_string();
        if string.chars().any(|c| !c.is_digit(10) && c != '.') {
            return None; // only digits and dot are allowed
        }
        if string.chars().filter(|c| *c == '.').count() > 1 {
            return None; // only one dot is allowed
        }
        println!("passed initial checks for string: {}", string);
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

    pub fn to_string(&self, ctx: &mut Context) -> String {
        match self {
            Number::Rational(r) => {
                if let Some(d) = helper_functions::decimal_from_rational(r) {
                    return d.to_string();
                }
                let float = helper_functions::float_from_rational(r, ctx);
                helper_functions::float_to_string_with_rounding(&float, 20)
            },
            Number::Float(f) => helper_functions::float_to_string_with_rounding(f, 20),
        }
    }

    pub fn get_debug_string(&self) -> String {
        match self {
            Number::Rational(r) => format!("rat({})", r),
            Number::Float(f) => format!("flt({})", f),
        }
    }

    pub fn error() -> Self {
        Self::Float(BigFloat::nan(None)) // todo check if this is marked as inexact
    }
}

// implement operations where there are several numbers as parameters
impl Number {
    pub fn plus(&self, other: &Self, ctx: &mut Context) -> Self {
        if let (Some(a), Some(b)) = (self.get_exact_rational(), other.get_exact_rational()) {
            Self::from(a + b)
        } else {
            let (a, b) = (self.get_float(ctx), other.get_float(ctx));
            Self::from(expr!(a + b, &mut *ctx))
        }
    }

    pub fn mul(&self, other: &Self, ctx: &mut Context) -> Self {
        if let (Some(a), Some(b)) = (self.get_exact_rational(), other.get_exact_rational()) {
            Self::from(a * b)
        } else {
            let (a, b) = (self.get_float(ctx), other.get_float(ctx));
            Self::from(expr!(a * b, &mut *ctx))
        }
    }

    pub fn div(&self, other: &Self, ctx: &mut Context) -> Self {
        if let (Some(a), Some(b)) = (self.get_exact_rational(), other.get_exact_rational()) {
            if b.is_zero() {
                return Self::error(); // handle division by zero
            }
            return Self::from(a / b);
        }
        let (a, b) = (self.get_float(ctx), other.get_float(ctx));
        if b.is_zero() {
            return Self::error(); // handle division by zero
        }
        Self::from(expr!(a / b, &mut *ctx)) // todo check whether this still stores is_inexact
    }

    pub fn pow(&self, other: &Self, ctx: &mut Context) -> Self {
        if let (Some(a), Some(b)) =
            (self.get_exact_rational(), other.get_exact_rational().and_then(|r| r.to_i32()))
        {
            return Self::from(a.pow(b));
        }
        let (a, b) = (self.get_float(ctx), other.get_float(ctx));
        Self::from(expr!(pow(a, b), &mut *ctx)) // todo check whether this still stores is_inexact
    }

    pub fn max(&self, other: &Self, ctx: &mut Context) -> Self {
        if let (Some(a), Some(b)) = (self.get_exact_rational(), other.get_exact_rational()) {
            return Self::from(a.max(b));
        }
        let (a, b) = (self.get_float(ctx), other.get_float(ctx));
        Self::from(a.max(&b)) // todo check whether this still stores is_inexact
    }
    pub fn min(&self, other: &Self, ctx: &mut Context) -> Self {
        if let (Some(a), Some(b)) = (self.get_exact_rational(), other.get_exact_rational()) {
            return Self::from(a.min(b));
        }
        let (a, b) = (self.get_float(ctx), other.get_float(ctx));
        Self::from(a.min(&b)) // todo check whether this still stores is_inexact
    }
}

// implement operations for just one number
impl Number {
    pub fn neg(&self, ctx: &mut Context) -> Self {
        match self {
            Self::Rational(r) => Self::from(-r),
            Self::Float(f) => Self::from(expr!(-f, &mut *ctx)),
        }
    }

    pub fn abs(&self, ctx: &mut Context) -> Self {
        if let Some(r) = self.get_exact_rational() {
            Self::from(r.abs())
        } else {
            Self::from(self.get_float(ctx).abs()) // todo check whether this still stores is_inexact
        }
    }

    pub fn floor(&self, ctx: &mut Context) -> Self {
        if let Some(r) = self.get_exact_rational() {
            Self::from(r.floor())
        } else {
            Self::from(self.get_float(ctx).floor())
        }
    }

    pub fn ceil(&self, ctx: &mut Context) -> Self {
        if let Some(r) = self.get_exact_rational() {
            Self::from(r.ceil())
        } else {
            Self::from(self.get_float(ctx).ceil())
        }
    }
    pub fn round(&self, ctx: &mut Context) -> Self {
        if let Some(r) = self.get_exact_rational() {
            Self::from(r.round())
        } else {
            let float = self.get_float(ctx);
            Self::from(float.round(ctx.precision(), ctx.rounding_mode()))
        }
    }
}

/// Macro to implement operations that convert the number to a float and apply the operation
macro_rules! float_op {
    ($op:ident) => {
        pub fn $op(&self, ctx: &mut Context) -> Self {
            let float = self.get_float(ctx);
            Number::from(expr!($op(float), &mut *ctx))
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

mod tests {
    use astro_float::BigFloat;
    use num_bigint::BigInt;
    use num_rational::BigRational;
    use std::str::FromStr;

    #[cfg(test)]
    fn creat_rational(a: impl Into<BigInt>, b: impl Into<BigInt>) -> BigRational {
        BigRational::new(a.into(), b.into())
    }

    #[test]
    fn test_rational_from_float() {
        let inputs_and_expected = [
            ("3.5", Some(creat_rational(7, 2))),
            (
                "1000000000000000000000000000000000000000000000000000000000000000000000000000021",
                Some(
                    BigRational::from_str(
                        "1000000000000000000000000000000000000000000000000000000000000000000000000000021",
                    )
                    .unwrap(),
                ),
            ),
            ("1.125", Some(creat_rational(9, 8))),
            ("inf", None),
            ("-inf", None),
        ];
        for (i, o) in inputs_and_expected {
            let float = BigFloat::from_str(i).unwrap();
            assert_eq!(crate::operations::helper_functions::rational_from_float(&float), o);
        }
    }

    #[test]
    fn test_round_scientific() {
        let inputs_and_expected = [
            ("1.23e10", 2, Some("1.2e10")),
            ("9.99e10", 2, Some("1e11")),
            ("1.19e10", 2, Some("1.2e10")),
            ("1.9999e10", 4, Some("2e10")),
            ("1.9999e10", 4, Some("2e10")),
            ("1e10", 4, Some("1e10")),
            ("11e10", 4, None), // there is no decimal point between the first and second digit
        ];
        for (input, decimals, expected) in inputs_and_expected {
            assert_eq!(
                crate::operations::helper_functions::round_scientific(input, decimals),
                expected.map(ToOwned::to_owned)
            );
        }
    }
}
