use crate::Element;
use crate::libraries::storing::{FormulaStore, InternalFunction};
use std::collections::{HashMap, HashSet};

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
    pub fn eval(&self) -> Option<f64> {
        match self {
            Element::Brackets(_)
            | Element::String(_)
            | Element::Variable(_)
            | Element::Function { .. }
            | Element::VariableOrFunction(_) => None,

            Element::Plus(elements) => {
                let mut sum = 0.0;
                for n in elements {
                    sum += n.eval()?;
                }
                Some(sum)
            },

            Element::Multiply(elements) => {
                let mut product = 1.0;
                for n in elements {
                    product *= n.eval()?;
                }
                Some(product)
            },

            Element::Negate(e) => e.eval().map(|n| -n),
            Element::Number(n) => Some(*n),
            Element::Pow(b, e) => Some(b.eval()?.powf(e.eval()?)),
        }
    }
    pub fn eval_with_formulas(&self, formulas: &HashMap<String, InternalFunction>) -> Result<f64, String> {
        match self {
            Element::Function { name, arguments } => {
                let Some(formula) = formulas.get(name) else {
                    return Err(format!("Function '{}' not defined", name));
                };
                let arguments_evaluated = arguments
                    .iter()
                    .map(|a| a.eval_with_formulas(formulas))
                    .collect::<Result<Vec<_>, String>>()?;
                formula
                    .call(arguments_evaluated)
                    .map_err(|e| format!("Error evaluating function '{}': {}", name, e))
            },

            Element::Brackets(_)
            | Element::String(_)
            | Element::Variable(_)
            | Element::VariableOrFunction(_) => {
                Err("Cannot evaluate brackets, strings or undefined variables".to_owned())
            },

            Element::Plus(elements) => {
                let mut sum = 0.0;
                for n in elements {
                    sum += n.eval_with_formulas(formulas)?;
                }
                Ok(sum)
            },

            Element::Multiply(elements) => {
                let mut product = 1.0;
                for n in elements {
                    product *= n.eval_with_formulas(formulas)?;
                }
                Ok(product)
            },

            Element::Negate(e) => e.eval_with_formulas(formulas).map(|n| -n),
            Element::Number(n) => Ok(*n),
            Element::Pow(b, e) => Ok(b.eval_with_formulas(formulas)?.powf(e.eval_with_formulas(formulas)?)),
        }
    }
    pub fn safe_eval_with_formulas(
        &self, formulas: &HashMap<String, InternalFunction>,
    ) -> Result<EvaluationResult, String> {
        let mut data_loss = false;
        match self {
            Element::Function { name, arguments } => {
                let Some(formula) = formulas.get(name) else {
                    return Err(format!("Function '{}' not defined", name));
                };
                let arguments_evaluated = arguments
                    .iter()
                    .map(|a| a.safe_eval_with_formulas(formulas))
                    .collect::<Result<Vec<_>, String>>()?;
                formula
                    .call(arguments_evaluated.iter().map(|r| r.value()).collect())
                    .map(|r| EvaluationResult::new(r, true))
                    .map_err(|e| format!("Error evaluating function '{}': {}", name, e))
            },

            Element::Brackets(_)
            | Element::String(_)
            | Element::Variable(_)
            | Element::VariableOrFunction(_) => {
                Err("Cannot evaluate brackets, strings or undefined variables".to_owned())
            },

            Element::Plus(elements) => {
                let mut sum = 0.0;
                for n in elements {
                    let prev = sum;
                    let e_result = n.safe_eval_with_formulas(formulas)?;
                    sum += e_result.value;
                    if e_result.possible_data_loss || sum - prev != e_result.value {
                        data_loss = true;
                    }
                }
                Ok(EvaluationResult::new(sum, data_loss))
            },

            Element::Multiply(elements) => {
                let mut product = 1.0;
                for n in elements {
                    let prev = product;
                    let e_result = n.safe_eval_with_formulas(formulas)?;
                    product *= e_result.value;
                    if e_result.possible_data_loss || product / prev != e_result.value {
                        data_loss = true;
                    }
                }
                Ok(EvaluationResult::new(product, data_loss))
            },

            Element::Negate(e) => e
                .safe_eval_with_formulas(formulas)
                .map(|n| EvaluationResult::new(-n.value, n.possible_data_loss)),
            Element::Number(n) => Ok(EvaluationResult::new(*n, data_loss)),
            Element::Pow(b, e) => {
                let res_1 = b.safe_eval_with_formulas(formulas)?;
                let res_2 = e.safe_eval_with_formulas(formulas)?;
                data_loss = res_1.possible_data_loss | res_2.possible_data_loss;

                let calc_result = res_1.value.powf(res_2.value);
                Ok(EvaluationResult::new(
                    calc_result,
                    data_loss || (calc_result.powf(1.0 / res_2.value) != res_1.value),
                ))
            },
        }
    }
}

impl FormulaStore {
    pub fn eval(&self, name: &str) -> Result<f64, String> {
        let mut formula = Element::parse(name).ok_or("Could not parse formula".to_owned())?;
        let dont_expand = self.internal_function_definitions.keys().cloned().collect();
        self.expand_formula(&mut formula, &dont_expand)?;
        formula.eval_with_formulas(&self.internal_function_definitions)
    }
    pub fn safe_eval(&self, name: &str) -> Result<EvaluationResult, String> {
        let mut formula = Element::parse(name).ok_or("Could not parse formula".to_owned())?;
        self.expand_formula(&mut formula, &self.internal_function_definitions.keys().cloned().collect())?;
        formula.safe_eval_with_formulas(&self.internal_function_definitions)
    }

    pub(crate) fn expand_formula(
        &self, formula: &mut Element, ignore_names: &HashSet<String>,
    ) -> Result<(), String> {
        let mut all_names = HashSet::new();

        loop {
            all_names.clear();
            formula.get_all_names(&mut all_names);
            all_names = all_names.difference(ignore_names).cloned().collect();

            if all_names.is_empty() {
                break;
            }
            for name in all_names.iter() {
                formula.insert_symbol(&self.get_insertion_element_expanded(name, ignore_names)?)?;
            }
        }

        Ok(())
    }
}

#[test]
fn test_eval_formula_store() {
    let mut store = FormulaStore::new_empty();
    store.add_symbol_from_string("f(x)=x^2").unwrap();
    store.add_symbol_from_string("a=4").unwrap();
    assert_eq!(store.eval("f(a)").unwrap(), 16.0);
}

#[test]
fn test_internal_function_definitions() {
    let mut store = FormulaStore::new_empty();
    store.define_default_internal_functions().unwrap();
    assert!(matches!(store.add_symbol_from_string("sin(x)=x"), Err(_)));

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

    store.add_symbol_from_string("f(x)=sin(x)").unwrap();
    assert!(matches!(store.add_symbol_from_string("g(x)=undefined(x)"), Err(_)));
    store.add_symbol_from_string("g(x)=f(x)+cos(x)").unwrap();

    assert_eq!(store.eval("f(0)").unwrap(), 0f64.sin());
    assert_eq!(store.eval("g(0)").unwrap(), 0f64.cos());

    store.add_symbol_from_string("good_sum(x,y)=sum(x,y)+sum(x,y)").unwrap();
    assert_eq!(store.eval("good_sum(1,2)").unwrap(), 1.0 + 2.0 + 1.0 + 2.0);
    store.add_symbol_from_string("weird_sum(x,y)=sum(x,y)+sum(x,y,1)").unwrap();
    assert_eq!(store.eval("weird_sum(1,2)").unwrap(), 1.0 + 2.0 + 1.0 + 2.0 + 1.0);
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
