use crate::Element;
use crate::parsing::signature::Signatures;
use std::collections::HashMap;

pub struct FormulaStore {
    signatures: Signatures,
    formulas: HashMap<String, Element>,
    parameter_mappings: HashMap<String, Option<Vec<String>>>,
}

impl FormulaStore {
    pub(crate) fn new_empty() -> Self {
        FormulaStore {
            signatures: Signatures::new_empty(),
            formulas: HashMap::new(),
            parameter_mappings: HashMap::new(),
        }
    }

    pub(crate) fn add_symbol_from_string(&mut self, string: &str) -> Result<(), String> {
        let (sig, def) = string.split_once("=").ok_or("String doesn't contain '='".to_owned())?;
        let sig = Element::parse(sig).ok_or("First formula could not be parsed")?;
        println!("Signature: {:?}", sig);
        let def = Element::parse(def).ok_or("Second formula could not be parsed")?;
        println!("Definition: {:?}", def);

        match self.signatures.add_symbol_from_function_signature_and_definition(sig, def.clone()) {
            Ok((name, arg_names)) => {
                self.formulas.insert(name.clone(), def);
                self.parameter_mappings.insert(name, arg_names);
                Ok(())
            },
            Err(err) => Err(format!("Could not add symbol: {}", err)),
        }
    }

    pub(crate) fn get_insertion_element(&self, name: &str) -> Option<InsertionElement> {
        Some(InsertionElement {
            name: name.to_string(),
            arguments: self.parameter_mappings.get(name)?.clone(),
            formula: self.formulas.get(name)?.clone(),
        })
    }

    pub(crate) fn get_signatures(&self) -> &Signatures {
        &self.signatures
    }

    pub(crate) fn get_formulas(&self) -> &HashMap<String, Element> {
        &self.formulas
    }
}

#[derive(Debug)]
pub struct InsertionElement {
    name: String,
    arguments: Option<Vec<String>>,
    formula: Element,
}

impl Element {
    pub(crate) fn insert_symbol(&mut self, insert: &InsertionElement) -> Result<(), String> {
        dbg!(insert);
        dbg!(&self);
        match self {
            Element::Brackets(elements)
            | Element::Plus(elements)
            | Element::Multiply(elements)
            | Element::Function { arguments: elements, .. } => {
                for e in elements {
                    e.insert_symbol(insert)?
                }
            },
            Element::Pow(a, b) => {
                a.insert_symbol(insert)?;
                b.insert_symbol(insert)?;
            },
            Element::Negate(x) => x.insert_symbol(insert)?,
            Element::Number(_)
            | Element::Variable(_)
            | Element::VariableOrFunction(_)
            | Element::String(_) => {},
        }
        if self.get_name().is_some_and(|n| n == insert.name) {
            let types_equal = if insert.arguments.is_some() {
                matches!(self, Element::Function { .. })
            } else {
                matches!(self, Element::Variable(_))
            };
            if !types_equal {
                return Err("Insertion element and formula don't match".to_owned());
            }
            if let Some(insert_args) = &insert.arguments {
                let Element::Function { arguments: self_arguments, .. } = self else {
                    panic!("This should be unreachable")
                };
                if insert_args.len() != self_arguments.len() {
                    dbg!(insert_args);
                    dbg!(self_arguments);
                    return Err("The function used in the formula and the supplied function have different parameter counts".to_owned());
                }
                let mut new_formula = insert.formula.clone();
                for (in_arg, val) in insert_args.iter().zip(self_arguments) {
                    new_formula.insert_symbol(&InsertionElement {
                        name: in_arg.clone(),
                        arguments: None,
                        formula: val.clone(),
                    })?
                }
                *self = new_formula;
            } else {
                *self = insert.formula.clone();
            }
        }
        Ok(())
    }
}

#[test]
fn test_insert_symbols() {
    let fun = Element::parse("x+y").unwrap();

    let mut formula = Element::parse("f(12, f(1, 2))").unwrap();
    formula.print_debug();
    formula
        .insert_symbol(&InsertionElement {
            name: "f".to_owned(),
            arguments: Some(vec!["x".to_owned(), "y".to_owned()]),
            formula: fun,
        })
        .unwrap();
    formula.print_debug();
}
