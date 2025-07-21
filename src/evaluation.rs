use crate::Element;
use crate::storing::FormulaStore;
use std::collections::HashSet;
use std::io::stdin;

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
}

impl FormulaStore {
    pub fn eval(&self, name: &str) -> Result<f64, String> {
        let mut formula = Element::parse(name).ok_or("Could not parse formula".to_owned())?;
        self.expand_formula(&mut formula, &HashSet::new())?;
        formula.eval().ok_or("Could not evaluate formula".to_owned())
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

pub fn run_formula_evaluator() {
    let mut store = FormulaStore::new_empty();

    loop {
        let mut line = "".to_owned();
        let _ = stdin().read_line(&mut line);
        line = line.trim().to_string();
        if line.contains('=') {
            println!("{:?}", store.add_symbol_from_string(&line));
        } else {
            println!("{:?}", store.eval(&line));
        }
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
