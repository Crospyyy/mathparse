use crate::Element;
use crate::libraries::parsing::signature::{Signatures, SymbolDeclarationData};
use std::collections::HashMap;

pub type InternalFunctionDefinition = fn(Vec<f64>) -> Result<f64, String>;

pub struct FormulaStore {
    signatures: Signatures,
    formulas: HashMap<String, Element>,
    parameter_mappings: HashMap<String, Option<Vec<String>>>,
    pub(super) internal_function_definitions: HashMap<String, InternalFunctionDefinition>,
}

impl FormulaStore {
    pub fn define_internal_function(
        &mut self, name: &str, definition: InternalFunctionDefinition,
    ) -> Result<(), String> {
        if self.internal_function_definitions.contains_key(name) {
            return Err(format!("Internal function definition with key `{}` already exists", name));
        }
        if self.formulas.contains_key(name) {
            return Err(format!("Formula definition with key `{}` already exists", name));
        }
        self.internal_function_definitions.insert(name.to_string(), definition);
        Ok(())
    }

    pub fn define_default_internal_functions(&mut self) -> Result<(), String> {
        self.define_internal_function("sin", |args| {
            expect_n_arguments(1, args.len())?;
            Ok(args[0].sin())
        })?;
        self.define_internal_function("cos", |args| {
            expect_n_arguments(1, args.len())?;
            Ok(args[0].cos())
        })?;
        self.define_internal_function("tan", |args| {
            expect_n_arguments(1, args.len())?;
            Ok(args[0].tan())
        })?;
        self.define_internal_function("sqrt", |args| {
            expect_n_arguments(1, args.len())?;
            Ok(args[0].sqrt())
        })?;
        self.define_internal_function("abs", |args| {
            expect_n_arguments(1, args.len())?;
            Ok(args[0].abs())
        })?;
        self.define_internal_function("log2", |args| {
            expect_n_arguments(1, args.len())?;
            Ok(args[0].log2())
        })?;
        self.define_internal_function("avg", |args| {
            expect_one_or_more_arguments(args.len())?;
            Ok(args.iter().sum::<f64>() / args.len() as f64)
        })?;
        self.define_internal_function("sum", |args| Ok(args.iter().sum::<f64>()))?;
        Ok(())
    }

    pub fn define_default_symbols(&mut self) -> Result<(), String> {
        self.add_variable_with_value("pi", std::f64::consts::PI)?;
        self.add_variable_with_value("e", std::f64::consts::E)?;
        Ok(())
    }
}

fn expect_n_arguments(expected: usize, got: usize) -> Result<(), String> {
    if expected == got {
        Ok(())
    } else {
        Err(format!(
            "Expected {} argument{}, got {}",
            expected,
            if expected == 1 { "" } else { "s" },
            got
        ))
    }
}

fn expect_one_or_more_arguments(got: usize) -> Result<(), String> {
    if got > 0 { Ok(()) } else { Err("Expected one or more arguments, got 0".to_owned()) }
}

impl FormulaStore {
    pub(crate) fn new_empty() -> Self {
        FormulaStore {
            signatures: Signatures::new_empty(),
            formulas: HashMap::new(),
            parameter_mappings: HashMap::new(),
            internal_function_definitions: HashMap::new(),
        }
    }

    pub(crate) fn add_symbol_from_string(&mut self, string: &str) -> Result<String, String> {
        let (sig, def) = string.split_once("=").ok_or("String doesn't contain '='".to_owned())?;
        let sig = Element::parse(sig).ok_or("First formula could not be parsed")?;
        let def = Element::parse(def).ok_or("Second formula could not be parsed")?;

        self.add_symbol_from_sig_and_def(sig, def)
    }

    pub fn add_variable_with_value(&mut self, name: &str, value: f64) -> Result<String, String> {
        let sig = Element::parse(name).ok_or("First formula could not be parsed")?;
        if !matches!(sig, Element::VariableOrFunction(_) | Element::Variable(_)) {
            return Err("Signature must be a variable".to_owned());
        }
        let def = Element::Number(value);
        self.add_symbol_from_sig_and_def(sig, def)
    }

    fn add_symbol_from_sig_and_def(
        &mut self, sig: Element, def: Element,
    ) -> Result<String, String> {
        let symbol_name_and_args = SymbolDeclarationData::from_formula(&sig)?;

        if self.internal_function_definitions.contains_key(symbol_name_and_args.get_name()) {
            return Err(format!(
                "Internal function definition with key `{}` already exists",
                symbol_name_and_args.get_name()
            ));
        }
        match self
            .signatures
            .add_symbol_from_function_signature_and_definition(symbol_name_and_args, def.clone())
        {
            Ok((name, arg_names)) => {
                self.formulas.insert(name.clone(), def);
                self.parameter_mappings.insert(name.clone(), arg_names);
                Ok(name)
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
    pub(crate) fn get_insertion_element_expanded(
        &self, name: &str,
    ) -> Result<InsertionElement, String> {
        let arguments =
            self.parameter_mappings.get(name).ok_or(format!("Symbol `{name}` not found"))?.clone();
        let mut formula =
            self.formulas.get(name).ok_or(format!("Symbol `{name}` not found"))?.clone();
        let original_formula = formula.clone();

        self.expand_formula(
            &mut formula,
            &arguments.as_ref().unwrap_or(&vec![]).iter().cloned().collect(),
        )?;

        Ok(InsertionElement { name: name.to_string(), arguments, formula })
    }

    #[cfg(test)]
    pub(crate) fn get_signatures(&self) -> &Signatures {
        &self.signatures
    }
}

#[test]
pub fn test_get_insertion_element_expanded() {
    let mut store = FormulaStore::new_empty();
    store.add_symbol_from_string("f(i)=i^2").unwrap();
    store.add_symbol_from_string("g(x)=f(x+1)").unwrap();
    store.add_symbol_from_string("h(x)=g(x)-3").unwrap();
    let insert = store.get_insertion_element_expanded("h").unwrap();
    dbg!(insert);
}
#[test]
pub fn test_insert_formula() {
    use crate::formula_short::{plus, var};
    let mut store = FormulaStore::new_empty();
    store.add_symbol_from_string("fun(f,x,y)=f(x,y)").unwrap();
    store.add_symbol_from_string("add(x,y)=x+y").unwrap();
    store.add_symbol_from_string("fun2(x,y)=fun(add, x, y)").unwrap();
    let insert = store.get_insertion_element_expanded("fun2").unwrap();
    assert_eq!(insert.name, "fun2");
    assert_eq!(insert.arguments, Some(vec!["x".to_string(), "y".to_string()]));
    assert_eq!(insert.formula, plus([var("x"), var("y")]));
    dbg!(insert);
}

#[derive(Debug)]
pub struct InsertionElement {
    name: String,
    arguments: Option<Vec<String>>,
    formula: Element,
}

impl InsertionElement {
    pub fn insert_param_values(&self, param_values: Vec<Element>) -> Result<Element, String> {
        if let Some(insert_args) = &self.arguments {
            let self_arguments = param_values;
            if insert_args.len() != self_arguments.len() {
                dbg!(insert_args);
                dbg!(self_arguments);
                return Err("The function used in the formula and the supplied function have different parameter counts".to_owned());
            }
            let mut new_formula = self.formula.clone();
            for (in_arg, val) in insert_args.iter().zip(self_arguments) {
                new_formula.insert_symbol(&InsertionElement {
                    name: in_arg.clone(),
                    arguments: None,
                    formula: val.clone(),
                })?
            }
            return Ok(new_formula);
        }
        Err("No arguments provided for insertion".to_owned())
    }
}

impl Element {
    pub(crate) fn insert_symbol(&mut self, insert: &InsertionElement) -> Result<(), String> {
        match self {
            Element::Brackets(elements)
            | Element::Plus(elements)
            | Element::Multiply(elements)
            | Element::Function { arguments: elements, .. } => {
                for e in elements {
                    e.insert_symbol(insert)?;
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
            // this is going to be the new logic
            match (&mut *self, &insert.arguments, &insert.formula) {
                (Element::Function { name, arguments }, params, insert_formula) => {
                    match (params, insert_formula) {
                        (None, Element::VariableOrFunction(new_name)) => {
                            *name = new_name.clone();
                        },
                        (Some(_), _) => {
                            *self = insert.insert_param_values(arguments.clone())?;
                        },
                        (..) => {
                            return Err(format!(
                                "Insertion element and formula don't match (self: {:?}, insert: {:?})",
                                self, insert
                            ));
                        },
                    }
                },
                (Element::Variable(name), None, _) => {
                    *self = insert.formula.clone();
                },
                (Element::VariableOrFunction(name), params, _) => {
                    if params.is_some() {
                        println!("Skipping this because there is no call yet");
                    } else {
                        *self = insert.formula.clone();
                    }
                },

                (..) => {
                    return Err(format!(
                        "Insertion element and formula don't match (self: {:?}, insert: {:?})",
                        self, insert
                    ));
                },
            }

            // // this is the logic that was used before
            // let types_equal = if insert.arguments.is_some() {
            //     matches!(self, Element::Function { .. })
            // } else {
            //     matches!(self, Element::Variable(_) | Element::VariableOrFunction(_))
            //         | (matches!(self, Element::Function { .. })
            //             && matches!(insert.formula, Element::VariableOrFunction(_)))
            // };
            // if !types_equal {
            //     return Err(format!(
            //         "Insertion element and formula don't match (self: {:?}, insert: {:?})",
            //         self, insert
            //     )
            //     .to_owned());
            // }
            // if let Some(insert_args) = &insert.arguments {
            //     let Element::Function { arguments: self_arguments, .. } = self else {
            //         panic!("This should be unreachable")
            //     };
            //     if insert_args.len() != self_arguments.len() {
            //         dbg!(insert_args);
            //         dbg!(self_arguments);
            //         return Err("The function used in the formula and the supplied function have different parameter counts".to_owned());
            //     }
            //     let mut new_formula = insert.formula.clone();
            //     for (in_arg, val) in insert_args.iter().zip(self_arguments) {
            //         new_formula.insert_symbol(
            //             &InsertionElement {
            //                 name: in_arg.clone(),
            //                 arguments: None,
            //                 formula: val.clone(),
            //             },
            //             found_function_elements_to_replace,
            //             replace_fun_args,
            //         )?
            //     }
            //     *self = new_formula;
            // } else if let (Element::Function { name, .. }, Element::VariableOrFunction(new_name)) =
            //     (&mut *self, &insert.formula)
            // {
            //     *name = new_name.clone();
            // } else {
            //     *self = insert.formula.clone();
            // }
        }
        match self {
            Element::Brackets(_)
            | Element::String(_)
            | Element::Number(_)
            | Element::Variable(_)
            | Element::VariableOrFunction(_) => {},
            Element::Plus(elements) | Element::Multiply(elements) => {
                for e in elements {
                    if let Element::VariableOrFunction(name) = e {
                        *e = Element::Variable(name.clone());
                    };
                }
            },
            Element::Pow(a, b) => {
                if let Element::VariableOrFunction(name) = &**a {
                    **a = Element::Variable(name.clone());
                }
                if let Element::VariableOrFunction(name) = &**b {
                    **b = Element::Variable(name.clone());
                }
            },
            Element::Negate(a) => {
                if let Element::VariableOrFunction(name) = &**a {
                    **a = Element::Variable(name.clone());
                }
            },
            Element::Function { .. } => {},
        }
        Ok(())
    }
}

#[test]
fn test_insert_symbols() {
    println!("### Test inserting symbols ###");

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

#[test]
fn test_storing() {
    println!("### Test storing formulas ###");

    let mut store = FormulaStore::new_empty();
    assert_eq!(store.add_symbol_from_string("f=123"), Ok("f".to_owned()));
    assert!(matches!(store.add_symbol_from_string("1=1"), Err(_)));
    assert!(matches!(store.add_symbol_from_string("f=1"), Err(_)));
    assert!(matches!(store.add_symbol_from_string("x"), Err(_)));

    assert!(matches!(store.add_symbol_from_string("g(l)=x^2"), Err(_)));
    assert_eq!(store.add_symbol_from_string("g(g)=g^2"), Ok("g".to_owned()));
    assert!(matches!(store.add_symbol_from_string("g=2"), Err(_)));

    assert_eq!(store.add_symbol_from_string("f2(f)=f*3"), Ok("f2".to_owned()));
    assert!(matches!(store.add_symbol_from_string("f3=f2()"), Err(_)));

    let result = store.eval("f");
    assert_eq!(result, Ok(123.0));
    assert!(matches!(store.eval("f()"), Err(_)));
    assert!(matches!(store.eval("g()"), Err(_)));
    assert_eq!(store.eval("g(2)"), Ok(4.0));
    assert_eq!(store.eval("f2(2)"), Ok(6.0));

    assert_eq!(store.add_symbol_from_string("add(a,b)=a+b"), Ok("add".to_owned()));
    assert_eq!(store.add_symbol_from_string("mul(a,b)=a*b"), Ok("mul".to_owned()));
    assert_eq!(store.add_symbol_from_string("div(a,b)=a/b"), Ok("div".to_owned()));
    assert!(matches!(store.eval("add(1,2)"), Ok(3.0)));
    assert_eq!(store.add_symbol_from_string("run(a, b, fun)=fun(a, b)"), Ok("run".to_owned()));
    assert_eq!(store.eval("run(1, 2, add)"), Ok(3.0));
    assert_eq!(store.eval("run(1, 2, mul)"), Ok(2.0));
    assert_eq!(store.eval("run(1, 2, div)"), Ok(0.5));
}
