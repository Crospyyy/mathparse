use crate::Number;
use crate::calculation::helper_functions::sin_radians;
use crate::expression_values::{ExpressionFunType, ExpressionNumType};
use crate::parsing::signature::ParamCount;
use astro_float::ctx::Context;
use astro_float::expr;

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

fn sin_with_radians(num: &Number, ctx: &mut Context) -> Number {
    if let Some(r) = num.get_exact_rational() {
        dbg!("calculate sin with rational");
        sin_radians(&r, ctx)
    } else {
        dbg!("calculate sin with float");
        let float = num.get_float(ctx);
        let mut result = expr!(sin(float * pi / 2), &mut *ctx);
        if float.inexact() {
            result.set_inexact(true)
        }
        Number::from(result)
    }
}

impl ExpressionFunType {
    pub(crate) fn get_function_single_arg(&self) -> Option<fn(&Number, &mut Context) -> Number> {
        match self {
            ExpressionFunType::Rem => None?,
            ExpressionFunType::Sin => Number::sin,
            ExpressionFunType::Asin => Number::asin,
            ExpressionFunType::Cos => Number::cos,
            ExpressionFunType::Acos => Number::acos,
            ExpressionFunType::Tan => Number::tan,
            ExpressionFunType::Atan => Number::atan,
            ExpressionFunType::Floor => Number::floor,
            ExpressionFunType::Round => Number::round,
            ExpressionFunType::Log2 => Number::log2,
            ExpressionFunType::SinWithRadians => sin_with_radians,
        }
        .into()
    }

    pub(crate) fn get_function_multiple_args(&self) -> Option<fn(Vec<Number>, &mut Context) -> Number> {
        match self {
            ExpressionFunType::Rem => Some(|args: Vec<Number>, ctx: &mut Context| {
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
            | ExpressionFunType::Log2
            | ExpressionFunType::SinWithRadians => None?,
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
            ExpressionFunType::SinWithRadians => ParamCount::Exactly(1),
        }
    }
}
