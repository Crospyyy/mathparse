use crate::calculation::helper_functions::float_to_exact_rational;
use crate::expression_values::ValueRange;
use crate::printing::FormattingOptions;
use crate::{Element, Number, NumberContext};
use astro_float::ctx::Context;
use astro_float::{BigFloat, Consts, Error, RoundingMode, expr};
use macros::formula_matches;
use num_rational::BigRational;
use num_traits::{Signed, ToPrimitive, Zero};
use regex::Regex;
use std::cmp::Ordering;
use std::ops::Neg;
use std::sync::LazyLock;

pub(crate) mod expression_functions;
mod helper_functions;
mod operations;
#[cfg(test)]
mod tests;
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
        formula_matches!(self, num(x)).is_some_and(|n| n.is_nan())
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

    pub(crate) fn is_in_value_range(&self, range: ValueRange) -> bool {
        todo!()
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
