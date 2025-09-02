use crate::calculation::helper_functions::float_to_exact_rational;
use crate::printing::FormattingOptions;
use crate::{Element, Number, NumberContext};
use astro_float::ctx::Context;
use astro_float::{BigFloat, Consts, Error, RoundingMode, expr};
use num_rational::BigRational;
use num_traits::{Signed, ToPrimitive, Zero};
use regex::Regex;
use std::cmp::Ordering;
use std::ops::Neg;
use std::sync::LazyLock;

pub(crate) mod expression_functions;
mod helper_functions;
mod operations;
mod trait_implementations;

static REGEX_NUMBER_UNDERSCORE_REMOVAL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d)_(\d)").unwrap());

pub fn create_default_context() -> NumberContext {
    NumberContext::new(
        1024,
        RoundingMode::ToEven,
        Consts::new().expect("Constants cache initialized"),
        -100000,
        100000,
    )
}

impl Element {
    pub(crate) fn is_nan(&self) -> bool {
        self.get_number_inner().is_some_and(|n| n.is_nan())
    }
}

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

#[cfg(test)]
mod tests {
    use crate::Number;
    use crate::calculation::helper_functions::{
        big_int_to_power_of_inv_of_big_int, power_rational_and_rational, rational_from_float,
    };
    use crate::calculation::{FormattingOptions, create_default_context};
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
