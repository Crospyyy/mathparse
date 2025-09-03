use crate::Number;
use crate::calculation::helper_functions::sin_radians;
use crate::expression_values::{ExpressionFunType, ExpressionNumType, FunctionExpression};
use crate::parsing::signature::ParamCount;
use astro_float::ctx::Context;
use astro_float::expr;
use std::rc::Rc;

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
    pub fn get_function(&self) -> FunctionExpression {
        todo!()
    }

    pub(crate) fn get_function_single_arg(&self) -> Option<Rc<dyn Fn(&Number, &mut Context) -> Number>> {
        Some(match self {
            ExpressionFunType::Rem => None?,
            ExpressionFunType::Sin => Rc::new(Number::sin),
            ExpressionFunType::Asin => Rc::new(Number::asin),
            ExpressionFunType::Cos => Rc::new(Number::cos),
            ExpressionFunType::Acos => Rc::new(Number::acos),
            ExpressionFunType::Tan => Rc::new(Number::tan),
            ExpressionFunType::Atan => Rc::new(Number::atan),
            ExpressionFunType::Floor => Rc::new(Number::floor),
            ExpressionFunType::Ceil => Rc::new(Number::ceil),
            ExpressionFunType::Round => Rc::new(Number::round),
            ExpressionFunType::Log2 => Rc::new(Number::log2),
            ExpressionFunType::SinWithRadians => Rc::new(sin_with_radians),
            ExpressionFunType::AssertValueRange(range) => {
                let r = *range;
                Rc::new(move |n, _c| if n.is_in_value_range(r) { n.clone() } else { Number::nan(None) })
            },
            ExpressionFunType::Custom(c) => c.get_fn_single_arg()?,
        })
    }

    pub(crate) fn get_function_multiple_args(
        &self,
    ) -> Option<Rc<dyn Fn(Vec<Number>, &mut Context) -> Number>> {
        macro_rules! args_check {
            ($args:expr,$len:literal, $name:ident) => {
                if $args.len() != $len {
                    eprintln!(
                        "{} function expects exactly {} arguments, but got {}",
                        stringify!($name),
                        $len,
                        $args.len()
                    );
                    return Number::nan(None);
                }
            };
        }
        Some(match self {
            ExpressionFunType::Rem => Rc::new(|args: Vec<Number>, ctx: &mut Context| {
                args_check!(args, 2, Rem);
                Number::rem(&args[0], &args[1], ctx)
            }),
            ExpressionFunType::Sin
            | ExpressionFunType::Asin
            | ExpressionFunType::Cos
            | ExpressionFunType::Acos
            | ExpressionFunType::Tan
            | ExpressionFunType::Atan
            | ExpressionFunType::Floor
            | ExpressionFunType::Ceil
            | ExpressionFunType::Round
            | ExpressionFunType::Log2
            | ExpressionFunType::SinWithRadians => None?,
            ExpressionFunType::Custom(c) => c.get_fn_multiple_args()?,
            ExpressionFunType::AssertValueRange(_) => None?,
        })
    }

    pub(crate) fn get_param_count(&self) -> ParamCount {
        match self {
            ExpressionFunType::Rem => ParamCount::Exactly(2),
            ExpressionFunType::Sin
            | ExpressionFunType::Asin
            | ExpressionFunType::Cos
            | ExpressionFunType::Acos
            | ExpressionFunType::Tan
            | ExpressionFunType::Atan
            | ExpressionFunType::Floor
            | ExpressionFunType::Ceil
            | ExpressionFunType::Round
            | ExpressionFunType::Log2
            | ExpressionFunType::SinWithRadians
            | ExpressionFunType::AssertValueRange(_) => ParamCount::Exactly(1),
            ExpressionFunType::Custom(c) => c.param_count,
        }
    }
}
