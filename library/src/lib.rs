use crate::new_calculation::Number;
use astro_float::ctx::Context;
use astro_float::{Consts, RoundingMode};
use crate::parsing::signature::ParamCount;

mod evaluation;
mod operations;
pub mod parsing;
mod printing;
pub mod storing;

#[derive(Debug, Clone, PartialEq)]
pub enum Element {
    // Unparsed elements
    /// Unparsed group of elements
    Brackets(Vec<Element>),
    /// Unparsed string
    String(String),

    // Unexpanded formula elements
    /// A function with a name and arguments
    Function { name: String, arguments: Vec<Element> },
    /// A variable with a name
    Variable(String),
    /// A variable, which could either be a number or a function
    VariableOrFunction(String),

    // Expanded formula elements
    /// A function with a stored evaluation expression
    FunctionWithExpression { arguments: Vec<Element>, param_count: ParamCount, expression: FunctionExpression },
    /// A number defined by an expression
    NumberWithExpression(fn(&mut Context) -> Number),
    /// List of elements to add together
    Plus(Vec<Element>),
    /// List of elements to multiply together
    Multiply(Vec<Element>),
    /// Exponential operation (base^exponent)
    Pow(Box<Element>, Box<Element>),
    /// Negation of an element (e.g., -x)
    Negate(Box<Element>),
    /// A number
    Number(Number),
}

pub fn create_default_context() -> Context {
    Context::new(
        1024,
        RoundingMode::ToEven,
        Consts::new().expect("Constants cache initialized"),
        -100000,
        100000,
    )
}

#[derive(Clone, Debug, PartialEq)]
pub enum FunctionExpression {
    SingleArgument(fn(&Number, &mut Context) -> Number),
    MultipleArguments(fn(&mut Context, Vec<Number>) -> Number),
}

#[allow(unused)]
mod formula_short {
    use crate::Element;
    use crate::new_calculation::Number;

    pub fn num(num: &str) -> Element {
        Element::Number(Number::from_string(num.to_owned()).unwrap())
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

    /// = element^-1
    pub fn inv(element: Element) -> Element {
        pow(element, neg(num("1")))
    }
}

mod new_calculation {
    use crate::create_default_context;
    use astro_float::ctx::Context;
    use astro_float::{BigFloat, Consts, Radix, RoundingMode, Word, expr};
    use num_bigint::BigInt;
    use num_rational::BigRational;
    use rust_decimal::Decimal;
    use rust_decimal::prelude::FromPrimitive;
    use std::ops::Not;
    use std::str::FromStr;

    #[derive(Clone, Debug, PartialEq)]
    pub enum Number {
        Rational(BigRational),
        Float(BigFloat),
    }

    #[allow(unused)]
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
                let rational = rational_from_string(&just_numbers)?;
                let factor = BigRational::from_integer(10.into()).pow(dot_index as i32);
                Some(Self::Rational(rational / factor))
            } else {
                Some(Self::Rational(rational_from_string(&string)?))
            }
        }

        pub fn is_exact(&self) -> bool {
            match self {
                Number::Rational(_) => true,
                Number::Float(float) => !float.inexact(),
            }
        }

        pub fn get_rational(&self) -> Option<BigRational> {
            match self {
                Number::Rational(r) => Some(r.clone()),
                Number::Float(f) => f.inexact().not().then_some(rational_from_float(f)?),
            }
        }

        pub(crate) fn get_float(&self, ctx: &mut Context) -> BigFloat {
            match self {
                Number::Rational(r) => float_from_rational(r, ctx),
                Number::Float(f) => f.clone(),
            }
        }

        pub fn to_string(&self, ctx: &mut Context) -> String {
            match self {
                Number::Rational(r) => {
                    if let Some(d) = decimal_from_rational(r) {
                        return d.to_string();
                    }
                    let float = float_from_rational(r, ctx);
                    float_to_string_with_rounding(&float, 20)
                },
                Number::Float(f) => float_to_string_with_rounding(f, 20),
            }
        }

        pub fn get_debug_string(&self) -> String {
            match self {
                Number::Rational(r) => format!("rat({})", r),
                Number::Float(f) => format!("flt({})", f),
            }
        }
    }

    fn rational_from_string(s: &str) -> Option<BigRational> {
        BigRational::from_str(s).ok()
    }

    fn decimal_from_rational(rational: &BigRational) -> Option<Decimal> {
        let num = Decimal::from_str(&rational.numer().to_string()).ok()?;
        let denom = Decimal::from_str(&rational.denom().to_string()).ok()?;
        Some(num / denom)
    }

    fn float_from_rational(rational: &BigRational, ctx: &mut Context) -> BigFloat {
        let a_float = BigFloat::from_str(&rational.numer().to_string()).unwrap();
        let b_float = BigFloat::from_str(&rational.denom().to_string()).unwrap();
        expr!(a_float / b_float, &mut *ctx)
    }

    fn rational_from_float(float: &BigFloat) -> Option<BigRational> {
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
            assert_eq!(rational_from_float(&float), o);
        }
    }

    fn round_scientific(str: &str, decimals: usize) -> Option<String> {
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

    fn float_to_string_with_rounding(float: &BigFloat, decimals: usize) -> String {
        let float_string = float.to_string();
        let scientific = round_scientific(&float_string, decimals).unwrap_or(float_string);
        let Ok(d) = Decimal::from_scientific(&scientific) else { return scientific };
        d.to_string()
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
            assert_eq!(round_scientific(input, decimals), expected.map(ToOwned::to_owned));
        }
    }

    #[test]
    fn calculation_workflow() {
        // ### Precise Calculation Workflow
        // - read the numbers as Rationals in the format "num/10^digits"
        // - do the calculations
        // - convert the result into a numerator/denominator pair
        // - calculate the result using Decimal

        // ### Imprecise Calculation Workflow
        // - read the number as BigFloat
        // - do the calculations
        // - convert the result to decimal using `convert_to_radix`
        // - trim the digits to n+1 digits before the decimal point
        // - round the last digit and apply changes to digits before if rounding up
        // - convert the result to a string
        // - print the result
    }

    #[test]
    fn test_rationals() {
        use num_rational::BigRational;
        let rational = BigRational::new(269.into(), 100.into());
        let rational2 = BigRational::new(101.into(), 100.into());
        let result = &rational + &rational2;
        println!("{}", result);
        let both = result.into_raw();
        let mut consts = Consts::new().expect("Constants cache initialized");
        let mut ctx = Context::new(
            1024,
            RoundingMode::ToEven,
            Consts::new().expect("Constants cache initialized"),
            -10000,
            10000,
        );
        let float = BigFloat::parse(&both.0.to_string(), Radix::Dec, 1024, RoundingMode::ToEven, &mut consts);
        let float2 =
            BigFloat::parse(&both.1.to_string(), Radix::Dec, 1024, RoundingMode::ToEven, &mut consts);
        let output = float.div(&float2, 1024, RoundingMode::ToEven);
        println!("Output: {}", output);
        println!("Exact: {}", !output.inexact());
    }

    #[test]
    fn test_decimals() {
        use rust_decimal::prelude::*;
        let num1 = 123;
        let num2 = 25;
        let num1_ = Decimal::from(num1);
        let num2_ = Decimal::from(num2);
        println!("{}", num1_ / num2_);
    }

    #[test]
    fn test_floats() {
        let mut ctx = Context::new(
            1024,
            RoundingMode::ToEven,
            Consts::new().expect("Constants cache initialized"),
            -10000,
            10000,
        );

        let input_num = BigFloat::from(1.4);
        let result = expr!(0.1 + 0.2, &mut ctx);

        let mut cc = Consts::new().expect("Constants cache initialized");

        println!("Inexact: {}", result.inexact());
        println!("Float: {}", result);
    }
}
