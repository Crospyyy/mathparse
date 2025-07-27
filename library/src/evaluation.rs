use crate::new_calculation::Number;
use crate::storing::FormulaStore;
use crate::{Element, FunctionExpression};
use astro_float::ctx::Context;
use std::collections::HashSet;
use astro_float::{Consts, RoundingMode};

#[derive(Debug, PartialEq)]
pub struct EvaluationResult {
    value: f64,
    possible_data_loss: bool,
}

impl EvaluationResult {
    pub fn new(value: f64, calculation_data_loss: bool) -> Self {
        EvaluationResult { value, possible_data_loss: calculation_data_loss }
    }

    pub fn no_loss(value: f64) -> Self {
        EvaluationResult { value, possible_data_loss: false }
    }

    pub fn with_loss(value: f64) -> Self {
        EvaluationResult { value, possible_data_loss: true }
    }

    pub fn is_lossy(&self) -> bool {
        self.possible_data_loss
    }

    pub fn value(&self) -> f64 {
        self.value
    }
}

impl Element {
    pub fn eval(&self, ctx: &mut Context) -> Option<Number> {
        match self {
            Element::Brackets(_)
            | Element::String(_)
            | Element::Variable(_)
            | Element::Function { .. }
            | Element::VariableOrFunction(_) => None,
            Element::Plus(elements) => {
                let mut sum = Number::from(0);
                for n in elements {
                    sum = sum.plus(&n.eval(ctx)?, ctx);
                }
                Some(sum)
            },
            Element::Multiply(elements) => {
                let mut product = Number::from(1);
                for n in elements {
                    product = product.mul(&n.eval(ctx)?, ctx);
                }
                Some(product)
            },
            Element::Negate(e) => e.eval(ctx).map(|n| n.neg(ctx)),
            Element::Number(n) => Some(n.clone()),
            Element::Pow(b, e) => Some(b.eval(ctx)?.pow(&e.eval(ctx)?, ctx)),
            Element::NumberWithExpression(fun) => Some(fun(ctx)),
            Element::FunctionWithExpression { arguments, expression } => match expression {
                FunctionExpression::SingleArgument(fun) => {
                    if arguments.len() != 1 {
                        return None;
                    }
                    Some(fun(&arguments[0].eval(ctx)?, ctx))
                },
                FunctionExpression::MultipleArguments(fun) => {
                    let args = arguments.iter().map(|a| a.eval(ctx)).collect::<Option<Vec<_>>>()?;
                    Some(fun(ctx, args))
                },
            },
        }
    }
    // pub fn eval_with_formulas(&self, formulas: &HashMap<String, InternalFunction>) -> Result<f64, String> {
    //     match self {
    //         Element::Function { name, arguments } => {
    //             let Some(formula) = formulas.get(name) else {
    //                 return Err(format!("Function '{}' not defined", name));
    //             };
    //             let arguments_evaluated = arguments
    //                 .iter()
    //                 .map(|a| a.eval_with_formulas(formulas))
    //                 .collect::<Result<Vec<_>, String>>()?;
    //             formula
    //                 .call(arguments_evaluated)
    //                 .map_err(|e| format!("Error evaluating function '{}': {}", name, e))
    //         },
    //
    //         Element::Brackets(_)
    //         | Element::String(_)
    //         | Element::Variable(_)
    //         | Element::VariableOrFunction(_) => {
    //             Err("Cannot evaluate brackets, strings or undefined variables".to_owned())
    //         },
    //
    //         Element::Plus(elements) => {
    //             let mut sum = 0.0;
    //             for n in elements {
    //                 sum += n.eval_with_formulas(formulas)?;
    //             }
    //             Ok(sum)
    //         },
    //
    //         Element::Multiply(elements) => {
    //             let mut product = 1.0;
    //             for n in elements {
    //                 product *= n.eval_with_formulas(formulas)?;
    //             }
    //             Ok(product)
    //         },
    //
    //         Element::Negate(e) => e.eval_with_formulas(formulas).map(|n| -n),
    //         Element::Number(n) => Ok(*n),
    //         Element::Pow(b, e) => Ok(b.eval_with_formulas(formulas)?.powf(e.eval_with_formulas(formulas)?)),
    //     }
    // }
    // pub fn safe_eval_with_formulas(
    //     &self, formulas: &HashMap<String, InternalFunction>,
    // ) -> Result<EvaluationResult, String> {
    //     let mut data_loss = false;
    //     match self {
    //         Element::Function { name, arguments } => {
    //             let Some(formula) = formulas.get(name) else {
    //                 return Err(format!("Function '{}' not defined", name));
    //             };
    //             let arguments_evaluated = arguments
    //                 .iter()
    //                 .map(|a| a.safe_eval_with_formulas(formulas))
    //                 .collect::<Result<Vec<_>, String>>()?;
    //             formula
    //                 .call(arguments_evaluated.iter().map(|r| r.value()).collect())
    //                 .map(|r| EvaluationResult::new(r, true))
    //                 .map_err(|e| format!("Error evaluating function '{}': {}", name, e))
    //         },
    //
    //         Element::Brackets(_)
    //         | Element::String(_)
    //         | Element::Variable(_)
    //         | Element::VariableOrFunction(_) => {
    //             Err("Cannot evaluate brackets, strings or undefined variables".to_owned())
    //         },
    //
    //         Element::Plus(elements) => {
    //             let mut sum = 0.0;
    //             for n in elements {
    //                 let prev = sum;
    //                 let e_result = n.safe_eval_with_formulas(formulas)?;
    //                 sum += e_result.value;
    //                 if e_result.possible_data_loss || sum - prev != e_result.value {
    //                     data_loss = true;
    //                 }
    //             }
    //             Ok(EvaluationResult::new(sum, data_loss))
    //         },
    //
    //         Element::Multiply(elements) => {
    //             let mut product = 1.0;
    //             for n in elements {
    //                 let prev = product;
    //                 let e_result = n.safe_eval_with_formulas(formulas)?;
    //                 product *= e_result.value;
    //                 if e_result.possible_data_loss || product / prev != e_result.value {
    //                     data_loss = true;
    //                 }
    //             }
    //             Ok(EvaluationResult::new(product, data_loss))
    //         },
    //
    //         Element::Negate(e) => e
    //             .safe_eval_with_formulas(formulas)
    //             .map(|n| EvaluationResult::new(-n.value, n.possible_data_loss)),
    //         Element::Number(n) => Ok(EvaluationResult::new(*n, data_loss)),
    //         Element::Pow(b, e) => {
    //             let res_1 = b.safe_eval_with_formulas(formulas)?;
    //             let res_2 = e.safe_eval_with_formulas(formulas)?;
    //             data_loss = res_1.possible_data_loss | res_2.possible_data_loss;
    //
    //             let calc_result = res_1.value.powf(res_2.value);
    //             Ok(EvaluationResult::new(
    //                 calc_result,
    //                 data_loss || (calc_result.powf(1.0 / res_2.value) != res_1.value),
    //             ))
    //         },
    //     }
    // }
}

impl FormulaStore {
    pub fn eval(&self, formula_str: &str) -> Result<Number, String> {
        dbg!("Parsing formula: {}", &formula_str);
        let mut formula = Element::parse(formula_str).map_err(|err| format!("Could not parse formula: {err}"))?;
        dbg!("Parsed formula: {:?}", &formula);
        self.expand_formula(&mut formula, &HashSet::new())?;
        dbg!("Expanded formula: {:?}", &formula);
        let mut ctx = Context::new(
            1024,
            RoundingMode::ToEven,
            Consts::new().unwrap(),
            -100000, 100000
        );
        formula.eval(&mut ctx).ok_or(format!("Could not evaluate formula: {}", formula_str))
    }

    pub(crate) fn expand_formula(
        &self, formula: &mut Element, ignore_names: &HashSet<String>,
    ) -> Result<(), String> {
        let mut all_names = HashSet::new();

        loop {
            let before = all_names.clone();
            all_names.clear();
            formula.get_all_names(&mut all_names);
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
}

impl Element {
    pub(crate) fn get_all_names(&self, names: &mut HashSet<String>) {
        match self {
            Element::Brackets(_) | Element::String(_) | Element::Number(_) => {},
            Element::Plus(e) | Element::Multiply(e) => {
                e.iter().for_each(|el| el.get_all_names(names));
            },
            Element::Pow(a, b) => {
                a.get_all_names(names);
                b.get_all_names(names);
            },
            Element::Negate(x) => x.get_all_names(names),
            Element::Function { name, arguments } => {
                names.insert(name.to_owned());
                arguments.iter().for_each(|arg| arg.get_all_names(names));
            },
            Element::Variable(name) | Element::VariableOrFunction(name) => {
                names.insert(name.to_owned());
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Element;
    use crate::evaluation::EvaluationResult;
    use crate::storing::FormulaStore;

    #[test]
    pub fn test_evaluation() {
        let inputs = [
            ("1+2", Some(3.0)),
            ("1+2*3", Some(7.0)),
            ("1+2*3-4/2", Some(5.0)),
            ("(1+2)*3", Some(9.0)),
            ("(1+2)*(3-4)", Some(-3.0)),
            ("x/x-x", None),
            // Potenzierungen
            ("2^3", Some(8.0)),
            ("2^3^2", Some(512.0)), // 2^(3^2) = 2^9
            ("(2^3)^2", Some(64.0)),
            ("-2^2", Some(-4.0)), // -(2^2)
            ("(-2)^2", Some(4.0)),
            // Negation
            ("-1", Some(-1.0)),
            ("--1", Some(1.0)),
            ("-(1+2)", Some(-3.0)),
            // Komplexere Ausdrücke
            ("1+2*3+4", Some(11.0)),
            ("(1+2)*(3+4)", Some(21.0)),
            ("2*3+4*5", Some(26.0)),
            ("2*(3+4)*5", Some(70.0)),
            // Division
            ("8/2", Some(4.0)),
            ("8/2/2", Some(2.0)),
            ("8/(2*2)", Some(2.0)),
            ("1/2+1/3", Some(1.0 / 2.0 + 1.0 / 3.0)),
            // Nullwerte
            ("0+0", Some(0.0)),
            ("0*5", Some(0.0)),
            ("5*0", Some(0.0)),
            ("0^2", Some(0.0)),
            // Dezimalzahlen
            ("1.5+2.5", Some(4.0)),
            ("3.14*2", Some(6.28)),
            ("10.5/2.5", Some(4.2)),
            // Fehlerfälle
            ("", None),
        ];
        println!("Starting formula evaluation tests");
        inputs.into_iter().for_each(|(i, o)| test_formula_evaluation(i, o));
    }

    fn test_formula_evaluation(input: &str, expected_output: Option<f64>) {
        println!("Testing formula evaluation for input: {}", input);
        let output = Element::parse(input).ok().as_ref().and_then(Element::eval);
        assert_eq!(output, expected_output);
    }

    #[test]
    fn test_eval_formula_store() {
        let mut store = FormulaStore::new_empty();
        store.add_symbol_from_string("f(x)=x^2", false).unwrap();
        store.add_symbol_from_string("a=4", false).unwrap();
        assert_eq!(store.eval("f(a)").unwrap(), 16.0);
        assert!(store.eval("f").is_err());
    }

    #[test]
    fn test_internal_function_definitions() {
        let mut store = FormulaStore::new_empty();
        store.define_default_internal_functions().unwrap();
        assert!(matches!(store.add_symbol_from_string("sin(x)=x", false), Err(_)));

        assert_eq!(store.eval("sin(123)"), Ok(123f64.sin()));
        assert_eq!(store.safe_eval("sin(123)"), Ok(EvaluationResult::with_loss(123f64.sin())));
        assert_eq!(store.eval("cos(123)"), Ok(123f64.cos()));
        assert_eq!(store.eval("tan(123)"), Ok(123f64.tan()));
        assert_eq!(store.eval("sqrt(123)"), Ok(123f64.sqrt()));
        assert_eq!(store.eval("abs(123)"), Ok(123f64.abs()));
        assert_eq!(store.eval("log2(123)"), Ok(123f64.log2()));
        assert_eq!(store.safe_eval("log2(123)"), Ok(EvaluationResult::with_loss(123f64.log2())));
        assert_eq!(store.eval("avg(1,2,3)"), Ok((1.0 + 2.0 + 3.0) / 3.0));
        assert_eq!(store.eval("max(1,2,3)"), Ok(3.0));
        assert_eq!(store.eval("min(1,2,3)"), Ok(1.0));
        assert_eq!(store.eval("sum(1,2,3)"), Ok(1.0 + 2.0 + 3.0));

        let functions_that_take_one_argument =
            ["sin", "cos", "tan", "sqrt", "abs", "log2", "floor", "ceil", "round"];
        for function in functions_that_take_one_argument {
            assert!(store.eval(&format!("{}(0)", function)).is_ok());
            assert!(store.eval(&format!("{}(0, 0)", function)).is_err());
        }
        let functions_that_take_one_or_more_arguments = ["avg", "max", "min"];
        for function in functions_that_take_one_or_more_arguments {
            assert!(store.eval(&format!("{}()", function)).is_err());
            assert!(store.eval(&format!("{}(0)", function)).is_ok());
            assert!(store.eval(&format!("{}(0,0)", function)).is_ok());
        }
        assert_eq!(store.eval("sum()"), Ok(0.0));

        // Test für floor
        assert_eq!(store.eval("floor(123.456)").unwrap(), 123.0);
        assert_eq!(store.eval("floor(-123.456)").unwrap(), -124.0);

        // Test für ceil
        assert_eq!(store.eval("ceil(123.456)").unwrap(), 124.0);
        assert_eq!(store.eval("ceil(-123.456)").unwrap(), -123.0);

        // Test für round
        assert_eq!(store.eval("round(123.456)").unwrap(), 123.0);
        assert_eq!(store.eval("round(123.789)").unwrap(), 124.0);
        assert_eq!(store.eval("round(-123.456)").unwrap(), -123.0);
        assert_eq!(store.eval("round(-123.789)").unwrap(), -124.0);
    }

    #[test]
    fn test_define_functions_with_internal_function_definitions() {
        let mut store = FormulaStore::new_empty();
        store.define_default_internal_functions().unwrap();

        store.add_symbol_from_string("f(x)=sin(x)", false).unwrap();
        assert!(matches!(store.add_symbol_from_string("g(x)=undefined(x)", false), Err(_)));
        store.add_symbol_from_string("g(x)=f(x)+cos(x)", false).unwrap();

        assert_eq!(store.eval("f(0)").unwrap(), 0f64.sin());
        assert_eq!(store.eval("g(0)").unwrap(), 0f64.cos());

        store.add_symbol_from_string("good_sum(x,y)=sum(x,y)+sum(x,y)", false).unwrap();
        assert_eq!(store.eval("good_sum(1,2)").unwrap(), 1.0 + 2.0 + 1.0 + 2.0);
        store.add_symbol_from_string("weird_sum(x,y)=sum(x,y)+sum(x,y,1)", false).unwrap();
        assert_eq!(store.eval("weird_sum(1,2)").unwrap(), 1.0 + 2.0 + 1.0 + 2.0 + 1.0);
    }
}
