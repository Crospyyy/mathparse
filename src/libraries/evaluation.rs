use crate::Element;
use crate::libraries::storing::FormulaStore;
use std::collections::HashSet;

#[derive(Debug)]
pub struct EvaluationResult {
    value: f64,
    calculation_data_loss: bool,
}

impl EvaluationResult {
    pub fn new(value: f64, calculation_data_loss: bool) -> Self {
        EvaluationResult { value, calculation_data_loss }
    }

    pub fn no_loss(value: f64) -> Self {
        EvaluationResult { value, calculation_data_loss: false }
    }

    pub fn with_loss(value: f64) -> Self {
        EvaluationResult { value, calculation_data_loss: true }
    }

    pub fn is_lossy(&self) -> bool {
        self.calculation_data_loss
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
    pub fn safe_eval(&self) -> Option<EvaluationResult> {
        let mut data_loss = false;
        match self {
            Element::Brackets(_)
            | Element::String(_)
            | Element::Variable(_)
            | Element::Function { .. }
            | Element::VariableOrFunction(_) => None,

            Element::Plus(elements) => {
                let mut sum = 0.0;
                for n in elements {
                    let prev = sum;
                    let e_result = n.safe_eval()?;
                    sum += e_result.value;
                    if e_result.calculation_data_loss || sum - prev != e_result.value {
                        data_loss = true;
                    }
                }
                Some(EvaluationResult::new(sum, data_loss))
            },

            Element::Multiply(elements) => {
                let mut product = 1.0;
                for n in elements {
                    let prev = product;
                    let e_result = n.safe_eval()?;
                    product *= e_result.value;
                    if e_result.calculation_data_loss || product / prev != e_result.value {
                        data_loss = true;
                    }
                }
                Some(EvaluationResult::new(product, data_loss))
            },

            Element::Negate(e) => {
                e.safe_eval().map(|n| EvaluationResult::new(-n.value, n.calculation_data_loss))
            },
            Element::Number(n) => Some(EvaluationResult::new(*n, data_loss)),
            Element::Pow(b, e) => {
                let res_1 = b.safe_eval()?;
                let res_2 = e.safe_eval()?;
                data_loss = res_1.calculation_data_loss | res_2.calculation_data_loss;

                let calc_result = res_1.value.powf(res_2.value);
                Some(EvaluationResult::new(
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
        self.expand_formula(&mut formula, &HashSet::new())?;
        formula.eval().ok_or("Could not evaluate formula".to_owned())
    }
    pub fn safe_eval(&self, name: &str) -> Result<EvaluationResult, String> {
        let mut formula = Element::parse(name).ok_or("Could not parse formula".to_owned())?;
        self.expand_formula(&mut formula, &HashSet::new())?;
        formula.safe_eval().ok_or("Could not evaluate formula".to_owned())
    }

    pub(crate) fn expand_formula(
        &self, formula: &mut Element, ignore_names: &HashSet<String>,
    ) -> Result<(), String> {
        let mut all_names = HashSet::new();
        formula.get_all_names(&mut all_names);
        all_names.retain(|name| !ignore_names.contains(name));
        while !all_names.is_empty() {
            for name in all_names.iter() {
                formula.insert_symbol(&self.get_insertion_element_expanded(name)?)?;
            }
            all_names.clear();
            formula.get_all_names(&mut all_names);
            all_names.retain(|name| !ignore_names.contains(name));
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
