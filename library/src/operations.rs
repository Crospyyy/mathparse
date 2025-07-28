use crate::new_calculation::{Number, float_to_exact_rational};
use astro_float::ctx::Context;
use astro_float::{BigFloat, expr};
use num_rational::BigRational;
use rust_decimal::prelude::{Signed, ToPrimitive, Zero};

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

impl Number {
    pub fn error() -> Self {
        Self::Float(BigFloat::nan(None)) // todo check if this is marked as inexact
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
