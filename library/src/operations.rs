use crate::new_calculation::Number;
use astro_float::ctx::Context;
use astro_float::{BigFloat, expr};
use num_rational::BigRational;
use rust_decimal::prelude::{Signed, ToPrimitive, Zero};

// implement operations where there are several numbers as parameters
impl Number {
    pub fn plus(&self, other: &Self, ctx: &mut Context) -> Self {
        if let (Some(a), Some(b)) = (self.get_rational(), other.get_rational()) {
            Self::Rational(a + b)
        } else {
            let (a, b) = (self.get_float(ctx), other.get_float(ctx));
            Self::Float(expr!(a + b, &mut *ctx))
        }
    }

    pub fn mul(&self, other: &Self, ctx: &mut Context) -> Self {
        if let (Some(a), Some(b)) = (self.get_rational(), other.get_rational()) {
            Self::Rational(a * b)
        } else {
            let (a, b) = (self.get_float(ctx), other.get_float(ctx));
            Self::Float(expr!(a * b, &mut *ctx))
        }
    }

    pub fn div(&self, other: &Self, ctx: &mut Context) -> Self {
        if let (Some(a), Some(b)) = (self.get_rational(), other.get_rational()) {
            if b.is_zero() {
                return Self::error(); // handle division by zero
            }
            return Self::Rational(a / b);
        }
        let (a, b) = (self.get_float(ctx), other.get_float(ctx));
        if b.is_zero() {
            return Self::error(); // handle division by zero
        }
        Self::Float(expr!(a / b, &mut *ctx)) // todo check whether this still stores is_inexact
    }

    pub fn pow(&self, other: &Self, ctx: &mut Context) -> Self {
        if let (Some(a), Some(b)) = (self.get_rational(), other.get_rational().and_then(|r| r.to_i32())) {
            return Self::Rational(a.pow(b));
        }
        let (a, b) = (self.get_float(ctx), other.get_float(ctx));
        Self::Float(expr!(pow(a, b), &mut *ctx)) // todo check whether this still stores is_inexact
    }

    pub fn max(&self, other: &Self, ctx: &mut Context) -> Self {
        if let (Some(a), Some(b)) = (self.get_rational(), other.get_rational()) {
            return Self::Rational(a.max(b));
        }
        let (a, b) = (self.get_float(ctx), other.get_float(ctx));
        Self::Float(a.max(&b)) // todo check whether this still stores is_inexact
    }
    pub fn min(&self, other: &Self, ctx: &mut Context) -> Self {
        if let (Some(a), Some(b)) = (self.get_rational(), other.get_rational()) {
            return Self::Rational(a.min(b));
        }
        let (a, b) = (self.get_float(ctx), other.get_float(ctx));
        Self::Float(a.min(&b)) // todo check whether this still stores is_inexact
    }
}

// implement operations for just one number
impl Number {
    pub fn neg(&self, ctx: &mut Context) -> Self {
        match self {
            Self::Rational(r) => Self::Rational(-r),
            Self::Float(f) => Self::Float(expr!(-f, &mut *ctx)),
        }
    }

    pub fn abs(&self, ctx: &mut Context) -> Self {
        if let Some(r) = self.get_rational() {
            Self::Rational(r.abs())
        } else {
            Self::Float(self.get_float(ctx).abs()) // todo check whether this still stores is_inexact
        }
    }

    pub fn floor(&self, ctx: &mut Context) -> Self {
        if let Some(r) = self.get_rational() {
            Self::Rational(r.floor())
        } else {
            Self::Float(self.get_float(ctx).floor())
        }
    }

    pub fn ceil(&self, ctx: &mut Context) -> Self {
        if let Some(r) = self.get_rational() {
            Self::Rational(r.ceil())
        } else {
            Self::Float(self.get_float(ctx).ceil())
        }
    }
    pub fn round(&self, ctx: &mut Context) -> Self {
        if let Some(r) = self.get_rational() {
            Self::Rational(r.round())
        } else {
            let float = self.get_float(ctx);
            Self::Float(float.round(ctx.precision(), ctx.rounding_mode()))
        }
    }
}

macro_rules! float_op {
    ($op:ident) => {
        pub fn $op(&self, ctx: &mut Context) -> Self {
            let float = self.get_float(ctx);
            Self::Float(expr!($op(float), &mut *ctx))
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
