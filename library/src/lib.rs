use astro_float::ctx::Context;
use astro_float::{BigFloat, Consts, Radix, RoundingMode, expr};
use rust_decimal::Decimal;

mod evaluation;
pub mod parsing;
mod printing;
pub mod storing;

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

#[allow(unused)]
mod formula_short {
    use crate::Element;

    pub fn num(num: impl Into<f64>) -> Element {
        Element::Number(num.into())
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
        pow(element, neg(num(1.0)))
    }
}

mod new_calculation {
    use astro_float::ctx::Context;
    use astro_float::{BigFloat, Consts, Radix, RoundingMode, expr};
    use num_rational::BigRational;
    use rust_decimal::Decimal;
    use rust_decimal::prelude::ToPrimitive;
    use std::ops::Not;

    enum Number {
        Rational(BigRational),
        Float(BigFloat),
    }

    #[allow(unused)]
    impl Number {
        fn from_string(str: &str) -> Self {
            Self::Rational(rational_from_string(str))
        }

        fn is_exact(&self) -> bool {
            match self {
                Number::Rational(_) => true,
                Number::Float(float) => !float.inexact(),
            }
        }

        fn get_rational(&self) -> Option<BigRational> {
            match self {
                Number::Rational(r) => Some(r.clone()),
                Number::Float(f) => {
                    f.inexact().not().then_some(())?;
                    Some(rational_from_float(f))
                },
            }
        }

        fn get_float(&self) -> BigFloat {
            match self {
                Number::Rational(r) => float_from_rational(r),
                Number::Float(f) => f.clone(),
            }
        }

        fn to_string(&self) -> String {
            match self {
                Number::Rational(r) => {
                    if let Some(d) = decimal_from_rational(r) {
                        return d.to_string();
                    }
                    let float = float_from_rational(r);
                    float_to_string_with_rounding(&float, 20)
                },
                Number::Float(f) => float_to_string_with_rounding(f, 20),
            }
        }

        fn neg(&self, ctx: &mut Context) -> Self {
            match self {
                Self::Rational(r) => Self::Rational(-r),
                Self::Float(f) => Self::Float(expr!(-f, &mut *ctx)),
            }
        }

        fn plus(&self, other: &Self, ctx: &mut Context) -> Self {
            if let (Some(a), Some(b)) = (self.get_rational(), other.get_rational()) {
                Self::Rational(a + b)
            } else {
                let (a, b) = (self.get_float(), other.get_float());
                Self::Float(expr!(a + b, &mut *ctx))
            }
        }

        fn mul(&self, other: &Self, ctx: &mut Context) -> Self {
            if let (Some(a), Some(b)) = (self.get_rational(), other.get_rational()) {
                Self::Rational(a * b)
            } else {
                let (a, b) = (self.get_float(), other.get_float());
                Self::Float(expr!(a * b, &mut *ctx))
            }
        }

        fn pow(&self, other: &Self, ctx: &mut Context) -> Self {
            if let (Some(a), Some(b)) = (self.get_rational(), other.get_rational().and_then(|r| r.to_i32())) {
                return Self::Rational(a.pow(b));
            }
            let (a, b) = (self.get_float(), other.get_float());
            Self::Float(expr!(pow(a, b), &mut *ctx))
        }
    }

    fn rational_from_string(s: &str) -> BigRational {
        todo!()
    }

    fn decimal_from_rational(rational: &BigRational) -> Option<Decimal> {
        todo!()
    }

    fn float_from_rational(rational: &BigRational) -> BigFloat {
        todo!()
    }

    fn rational_from_float(float: &BigFloat) -> BigRational {
        todo!()
    }

    fn float_to_scientific_rounded_to(float: &BigFloat, decimals: usize) -> String {
        todo!()
    }

    fn float_to_string_with_rounding(float: &BigFloat, decimals: usize) -> String {
        let scientific = float_to_scientific_rounded_to(&float, decimals);
        if let Ok(d) = Decimal::from_scientific(&scientific) {
            return d.to_string();
        }
        scientific
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
