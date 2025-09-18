use crate::calculation::create_context;
use crate::expression_values::FunctionExpression;
use crate::storing::FormulaStore;
use crate::{Benchmark, Element, Number, RoundingMode, benchmark, create_default_context};
use astro_float::BigFloat;
use astro_float::ctx::Context;
use num_rational::BigRational;
use std::collections::HashSet;
use std::ops::RangeInclusive;

impl Element {
    pub fn eval(&self, ctx: &mut Context) -> Option<Number> {
        match self {
            Element::Brackets(_)
            | Element::String(_)
            | Element::Variable(_)
            | Element::Function { .. }
            | Element::VariableOrFunction(_) => None,
            Element::Plus(elements) => {
                let values: Vec<_> = elements.iter().map(|e| e.eval(ctx)).collect::<Option<_>>()?;
                if let Some(nan) = values.iter().find(|v| v.is_nan()) {
                    return Some(nan.clone());
                }
                let product = values.iter().fold(Number::from(0), |acc, n| acc.plus(n, ctx));
                Some(product)
            },
            Element::Multiply(elements) => {
                let values: Vec<_> = elements.iter().map(|e| e.eval(ctx)).collect::<Option<_>>()?;
                if let Some(nan) = values.iter().find(|v| v.is_nan()) {
                    return Some(nan.clone());
                }
                if values.iter().any(|v| v == 0) {
                    return Some(Number::from(0));
                }
                let product = values.iter().fold(Number::from(1), |acc, n| acc.mul(n, ctx));
                Some(product)
            },
            Element::Negate(e) => e.eval(ctx).map(|n| n.neg()),
            Element::Number(n) => Some(n.clone()),
            Element::Pow(b, e) => Some(b.eval(ctx)?.pow(&e.eval(ctx)?, ctx)),
            Element::NumberWithExpression { expr_value } => Some(expr_value.get_function()(ctx)),
            Element::FunctionWithExpression { arguments, expr_value } => {
                let count = expr_value.get_param_count();
                if !count.number_would_be_valid(arguments.len()) {
                    return None;
                }
                match expr_value.get_function() {
                    FunctionExpression::SingleArgument(fun) => Some(fun(&arguments[0].eval(ctx)?, ctx)),
                    FunctionExpression::MultipleArguments(fun) => {
                        let args = arguments.iter().map(|a| a.eval(ctx)).collect::<Option<Vec<_>>>()?;
                        Some(fun(args, ctx))
                    },
                }
            },
        }
    }

    pub(crate) fn get_all_unexpanded_names(&self, names: &mut HashSet<String>) {
        match self {
            Element::Brackets(_)
            | Element::String(_)
            | Element::Number(_)
            | Element::NumberWithExpression { .. }
            | Element::FunctionWithExpression { .. } => {},
            Element::Plus(e) | Element::Multiply(e) => {
                e.iter().for_each(|el| el.get_all_unexpanded_names(names));
            },
            Element::Pow(a, b) => {
                a.get_all_unexpanded_names(names);
                b.get_all_unexpanded_names(names);
            },
            Element::Negate(x) => x.get_all_unexpanded_names(names),
            Element::Function { name, arguments } => {
                names.insert(name.to_owned());
                arguments.iter().for_each(|arg| arg.get_all_unexpanded_names(names));
            },
            Element::Variable(name) | Element::VariableOrFunction(name) => {
                names.insert(name.to_owned());
            },
        }
    }
}

impl FormulaStore {
    pub fn eval(&self, formula_str: &str, ctx: &mut Context) -> Result<Number, String> {
        self.eval_with_benchmark(formula_str, ctx, &mut Benchmark::new())
    }

    pub fn eval_with_benchmark(
        &self, formula_str: &str, ctx: &mut Context, benchmark: &mut Benchmark,
    ) -> Result<Number, String> {
        let mut inner_bench = benchmark.new_sub_bench();
        let mut formula = Element::parse_benched(formula_str, &mut inner_bench)
            .map_err(|err| format!("Could not parse formula: {err}"))?;
        benchmark.add_task_with_benchmark("Parsing", inner_bench);

        dbg!(formula.get_string(&mut create_default_context()));
        benchmark!(benchmark, formula.optimize_and_reduce(), "Formula Optimization");
        dbg!(formula.get_string(&mut create_default_context()));
        benchmark!(benchmark, self.expand_formula(&mut formula, &HashSet::new())?, "Expansion");
        dbg!(formula.get_string(&mut create_default_context()));
        benchmark!(benchmark, formula.optimize_and_reduce(), "Formula Optimization");
        dbg!(formula.get_string(&mut create_default_context()));

        let result = benchmark!(
            benchmark,
            formula.eval(ctx).ok_or(format!("Could not evaluate formula: {}", formula_str))?,
            "Evaluation"
        );

        Ok(result)
    }

    pub(crate) fn expand_formula(
        &self, formula: &mut Element, ignore_names: &HashSet<String>,
    ) -> Result<(), String> {
        let mut all_names = HashSet::new();

        loop {
            let before = all_names.clone();
            all_names.clear();
            formula.get_all_unexpanded_names(&mut all_names);
            all_names = all_names.difference(ignore_names).cloned().collect();

            if all_names.is_empty() {
                break;
            }
            if all_names == before {
                return Err("Formula cannot be expanded".to_owned());
            }
            for name in all_names.iter() {
                formula.insert_symbol(&self.get_insertion_element_expanded(name, ignore_names)?)?;
            }
        }

        Ok(())
    }

    pub fn eval_dynamic_precision(
        &mut self, formula_str: &str, precision_range_bits: RangeInclusive<u32>,
    ) -> Result<DynamicResult, String> {
        let min_precision = *precision_range_bits.start();
        let mut precision = min_precision;
        let mut ctx = create_context(precision as usize);
        let first_result = self.eval(formula_str, &mut ctx)?;

        let mut last_rounded = match first_result {
            Number::Rational(r) => return Ok(DynamicResult::Exact(r)),
            Number::Float(f) => f,
        };
        precision *= 2;
        while precision <= *precision_range_bits.end() {
            let result = self.eval(formula_str, &mut ctx)?;
            let mut rounded = match result {
                Number::Float(f) => f,
                Number::Rational(r) => {
                    return Ok(DynamicResult::Exact(r));
                },
            }
            .round(min_precision as usize, RoundingMode::ToEven);
            rounded.set_inexact(true);

            if last_rounded == rounded {
                return Ok(DynamicResult::Checked { num: rounded, precision });
            }

            last_rounded = rounded;
            precision *= 2;
        }
        Ok(DynamicResult::ReachedLimit(last_rounded))
    }
}

pub enum DynamicResult {
    Exact(BigRational),
    Checked { num: BigFloat, precision: u32 },
    ReachedLimit(BigFloat),
}

#[cfg(test)]
mod tests {
    use crate::calculation::create_default_context;
    use crate::expression_values::{ExpressionFunType, ExpressionNumType};
    use crate::formula_short::{fun_expr, inv, mul, num, num_expr};
    use crate::storing::FormulaStore;
    use crate::{Element, Number};
    use astro_float::ctx::Context;
    use astro_float::{Error, expr};

    #[test]
    fn test_formula_evaluation() {
        let mut ctx = create_default_context();
        let inputs = [
            ("0/0", Some(Number::nan(Error::DivisionByZero.into()))),
            ("1/0", Some(Number::nan(Error::DivisionByZero.into()))),
            ("1+2", Some(3.into())),
            ("1+2*3", Some(7.into())),
            ("1+2*3-4/2", Some(5.into())),
            ("(1+2)*3", Some(9.into())),
            ("(1+2)*(3-4)", Some((-3).into())),
            ("x/x-x", None),
            // Potenzierungen
            ("2^3", Some(8.into())),
            ("2^3^2", Some(512.into())), // 2^(3^2) = 2^9
            ("(2^3)^2", Some(64.into())),
            ("-2^2", Some((-4).into())), // -(2^2)
            ("(-2)^2", Some(4.into())),
            ("16^(1/2)", Some(4.into())),
            // Negation
            ("-1", Some((-1).into())),
            ("--1", Some(1.into())),
            ("-(1+2)", Some((-3).into())),
            // Komplexere Ausdrücke
            ("1+2*3+4", Some(11.into())),
            ("(1+2)*(3+4)", Some(21.into())),
            ("2*3+4*5", Some(26.into())),
            ("2*(3+4)*5", Some(70.into())),
            // Division
            ("8/2", Some(4.into())),
            ("8/2/2", Some(2.into())),
            ("8/(2*2)", Some(2.into())),
            (
                "1/2+1/3",
                Some(
                    Number::from(1)
                        .div(&Number::from(2), &mut ctx)
                        .plus(&Number::from(1).div(&Number::from(3), &mut ctx), &mut ctx),
                ),
            ),
            // Nullwerte
            ("0+0", Some(0.into())),
            ("0*5", Some(0.into())),
            ("5*0", Some(0.into())),
            ("0^2", Some(0.into())),
            // Dezimalzahlen
            ("1.5+2.5", Some(4.into())),
            ("3.14*2", Some(Number::from_string("3.14").unwrap().mul(&Number::from(2), &mut ctx))),
            (
                "10.5/2.5",
                Some(
                    Number::from_string("10.5").unwrap().div(&Number::from_string("2.5").unwrap(), &mut ctx),
                ),
            ),
            // Fehlerfälle
            ("", None),
        ];
        println!("Starting formula evaluation tests");
        inputs.into_iter().for_each(|(i, o)| {
            println!("Testing formula evaluation for input: {}", i);
            let mut ctx1 = Context::new(
                1024,
                astro_float::RoundingMode::ToEven,
                astro_float::Consts::new().unwrap(),
                -100000,
                100000,
            );
            let output = Element::parse(i).ok().as_ref().and_then(|e| e.eval(&mut ctx1));
            assert_eq!(output, o);
        });
    }

    #[test]
    fn test_eval_formula_store() {
        let mut store = FormulaStore::new_empty();
        let mut ctx = create_default_context();
        store.add_symbol_from_string("f(x)=x^2", false).unwrap();
        store.add_symbol_from_string("a=4", false).unwrap();
        assert_eq!(store.eval("f(a)", &mut ctx).unwrap(), 16);
        assert!(store.eval("f", &mut ctx).is_err());
    }

    #[test]
    fn test_expression_functions() {
        let mut store = FormulaStore::new_empty();
        let mut ctx = create_default_context();

        store.define_default_symbols().unwrap();

        // test signatures
        fn generate_fn_call(fun_name: &str, arg_count: usize) -> String {
            format!("{}({})", fun_name, vec!["0"; arg_count].join(","))
        }
        let mut test_with_multiple_arg_counts = |function_names: &[&str], expected: fn(usize) -> bool| {
            for function in function_names {
                for i in 0..5 {
                    println!("Testing function: {} with {} arguments", function, i);
                    assert_eq!(store.eval(&generate_fn_call(function, i), &mut ctx).is_ok(), expected(i));
                }
            }
        };

        let functions_that_take_one_argument =
            ["sin", "cos", "tan", "sqrt", "abs", "log2", "log10", "ln", "floor", "ceil", "round"];
        let functions_that_take_zero_or_more_arguments = ["sum"];
        let functions_that_take_one_or_more_arguments = ["avg", "max", "min", "median"];

        test_with_multiple_arg_counts(&functions_that_take_one_argument, |i| i == 1);
        test_with_multiple_arg_counts(&functions_that_take_zero_or_more_arguments, |_| true);
        test_with_multiple_arg_counts(&functions_that_take_one_or_more_arguments, |i| i >= 1);

        macro_rules! test_eval {
            ($input:literal, $output:expr) => {
                assert_eq!(store.eval($input, &mut ctx), Ok($output))
            };
        }
        test_eval!("rem(-2, 2)", Number::from(0));
        test_eval!("rem(-1, 2)", Number::from(1));
        test_eval!("rem(0, 2)", Number::from(0));
        test_eval!("rem(1, 2)", Number::from(1));
        test_eval!("rem(2, 2)", Number::from(0));
        test_eval!(
            "sin(123)",
            fun_expr(
                ExpressionFunType::SinWithRadians,
                [mul([
                    fun_expr(
                        ExpressionFunType::Rem,
                        [mul([num(123), inv(num_expr(ExpressionNumType::Pi))]), num(2),]
                    ),
                    num(2),
                ])]
            )
            .eval(&mut ctx)
            .unwrap()
        );
        test_eval!("cos(123)", Number::Float(expr!(cos(123), &mut ctx)));
        test_eval!("tan(123)", Number::Float(expr!(tan(123), &mut ctx)));
        test_eval!("sqrt(123)", Number::Float(expr!(sqrt(123), &mut ctx)));
        test_eval!("abs(-123)", 123.into());
        test_eval!("log2(123)", Number::Float(expr!(log2(123), &mut ctx)));
        test_eval!("avg(1,2,3)", 2.into());
        test_eval!("max(1,2,3)", 3.into());
        test_eval!("min(1,2,3)", 1.into());
        test_eval!("sum(1,2,3)", 6.into());
        test_eval!("median(1,2,3)", 2.into());
        test_eval!("median(1,2,3,4)", Number::from_string("2.5").unwrap());
        test_eval!("median(2,3,4,1)", Number::from_string("2.5").unwrap());
        test_eval!("median(3,4,1,2)", Number::from_string("2.5").unwrap());
        test_eval!("median(4,1,2,3)", Number::from_string("2.5").unwrap());
        test_eval!("median(4,1,2,3)", Number::from_string("2.5").unwrap());

        test_eval!("sum()", 0.into());
        test_eval!("sum(0)", 0.into());
        test_eval!("sum(0,0)", 0.into());

        test_eval!("floor(123.456)", 123.into());
        test_eval!("floor(-123.456)", Number::from(-124));

        test_eval!("ceil(123.456)", Number::from(124));
        test_eval!("ceil(-123.456)", Number::from(-123));

        test_eval!("round(123.456)", Number::from(123));
        test_eval!("round(123.789)", Number::from(124));
        test_eval!("round(-123.456)", Number::from(-123));
        test_eval!("round(-123.789)", Number::from(-124));
    }

    #[test]
    fn test_define_functions_with_expression_function_definitions() {
        let mut store = FormulaStore::new_empty();
        let ctx = &mut create_default_context();
        store.define_default_symbols().unwrap();

        store.add_symbol_from_string("f(x)=sin(x)", false).unwrap();
        assert!(matches!(store.add_symbol_from_string("g(x)=undefined(x)", false), Err(_)));
        store.add_symbol_from_string("g(x)=f(x)+cos(x)", false).unwrap();

        assert_eq!(store.eval("f(0)", ctx).unwrap(), Number::from_string("0").unwrap().sin(ctx));
        assert_eq!(
            store.eval("g(0)", ctx).unwrap(),
            Number::from_string("0").unwrap().sin(ctx).plus(&Number::from_string("0").unwrap().cos(ctx), ctx)
        );

        store.add_symbol_from_string("good_sum(x,y)=sum(x,y)+sum(x,y)", false).unwrap();
        assert_eq!(store.eval("good_sum(1,2)", ctx).unwrap(), 1 + 2 + 1 + 2);
        store.add_symbol_from_string("weird_sum(x,y)=sum(x,y)+sum(x,y,1)", false).unwrap();
        assert_eq!(store.eval("weird_sum(1,2)", ctx).unwrap(), 1 + 2 + 1 + 2 + 1);
    }
}
