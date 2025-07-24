use crate::libraries::parsing::signature::{ParamCount, Signature, Signatures, SymbolDeclarationData};
use crate::Element;
use std::collections::{HashMap, HashSet};

impl InternalFunction {
    pub fn new_with_one_parameter(definition: fn(f64) -> f64) -> Self {
        Self::OneParameter(definition)
    }
    pub fn new_with_n_parameters(n: usize, definition: fn(Vec<f64>) -> f64) -> Self {
        Self::NParameters(n, definition)
    }
    pub fn new_with_n_or_more_parameters(n: usize, definition: fn(Vec<f64>) -> f64) -> Self {
        Self::NOrMoreParameters(n, definition)
    }

    pub fn is_param_count_valid(&self, param_count: usize) -> bool {
        match self {
            InternalFunction::OneParameter(_) => param_count == 1,
            InternalFunction::NParameters(n, _) => param_count == *n,
            InternalFunction::NOrMoreParameters(n, _) => param_count >= *n,
        }
    }

    fn verify_parameter_count(&self, param_count: usize) -> Result<(), String> {
        self.is_param_count_valid(param_count).then_some(()).ok_or(match self {
            InternalFunction::OneParameter(_) => {
                format!("Expected one argument, got {}", param_count)
            },
            InternalFunction::NParameters(n, _) => {
                format!("Expected {} argument{}, got {}", *n, if *n == 1 { "" } else { "s" }, param_count)
            },
            InternalFunction::NOrMoreParameters(n, _) => {
                format!("Expected {} or more arguments, got {}", *n, param_count)
            },
        })
    }

    pub fn call(&self, args: Vec<f64>) -> Result<f64, String> {
        self.verify_parameter_count(args.len())?;
        Ok(match self {
            InternalFunction::OneParameter(fun) => fun(args[0]),
            InternalFunction::NParameters(_, fun) => fun(args),
            InternalFunction::NOrMoreParameters(_, fun) => fun(args),
        })
    }

    pub fn get_param_count(&self) -> usize {
        match self {
            InternalFunction::OneParameter(_) => 1,
            InternalFunction::NParameters(n, _) | InternalFunction::NOrMoreParameters(n, _) => *n,
        }
    }

    pub fn get_signature(&self) -> Signature {
        match self {
            InternalFunction::OneParameter(_) => Signature::InternalFunction(ParamCount::Exactly(1)),
            InternalFunction::NParameters(n, _) => Signature::InternalFunction(ParamCount::Exactly(*n)),
            InternalFunction::NOrMoreParameters(n, _) => Signature::InternalFunction(ParamCount::AtLeast(*n)),
        }
    }
}

#[derive(Debug)]
pub enum InternalFunction {
    OneParameter(fn(f64) -> f64),
    NParameters(usize, fn(Vec<f64>) -> f64),
    NOrMoreParameters(usize, fn(Vec<f64>) -> f64),
}

pub struct FormulaStore {
    signatures: Signatures,
    formulas: HashMap<String, Element>,
    parameter_mappings: HashMap<String, Option<Vec<String>>>,
    pub(super) internal_function_definitions: HashMap<String, InternalFunction>,
}

impl FormulaStore {
    pub fn define_internal_function(
        &mut self, name: &str, internal_function: InternalFunction,
    ) -> Result<(), String> {
        if self.internal_function_definitions.contains_key(name) {
            return Err(format!("Internal function definition with key `{}` already exists", name));
        }
        if self.formulas.contains_key(name) {
            return Err(format!("Formula definition with key `{}` already exists", name));
        }
        self.internal_function_definitions.insert(name.to_string(), internal_function);
        Ok(())
    }

    pub fn define_default_internal_functions(&mut self) -> Result<(), String> {
        self.define_internal_function("sin", InternalFunction::new_with_one_parameter(f64::sin))?;
        self.define_internal_function("cos", InternalFunction::new_with_one_parameter(f64::cos))?;
        self.define_internal_function("tan", InternalFunction::new_with_one_parameter(f64::tan))?;

        self.define_internal_function("asin", InternalFunction::new_with_one_parameter(f64::asin))?;
        self.define_internal_function("acos", InternalFunction::new_with_one_parameter(f64::acos))?;
        self.define_internal_function("atan", InternalFunction::new_with_one_parameter(f64::atan))?;

        self.define_internal_function("sqrt", InternalFunction::new_with_one_parameter(f64::sqrt))?;
        self.define_internal_function("abs", InternalFunction::new_with_one_parameter(f64::abs))?;
        self.define_internal_function("log2", InternalFunction::new_with_one_parameter(f64::log2))?;
        self.define_internal_function("log10", InternalFunction::new_with_one_parameter(f64::log10))?;
        self.define_internal_function("ln", InternalFunction::new_with_one_parameter(f64::ln))?;
        self.define_internal_function("floor", InternalFunction::new_with_one_parameter(f64::floor))?;
        self.define_internal_function("ceil", InternalFunction::new_with_one_parameter(f64::ceil))?;
        self.define_internal_function("round", InternalFunction::new_with_one_parameter(f64::round))?;

        self.define_internal_function(
            "avg",
            InternalFunction::new_with_n_or_more_parameters(1, |args| {
                args.iter().sum::<f64>() / args.len() as f64
            }),
        )?;
        self.define_internal_function(
            "max",
            InternalFunction::new_with_n_or_more_parameters(1, |args| {
                args.iter().copied().max_by(|a, b| a.total_cmp(b)).unwrap_or(0.0)
            }),
        )?;
        self.define_internal_function(
            "min",
            InternalFunction::new_with_n_or_more_parameters(1, |args| {
                args.iter().copied().min_by(|a, b| a.total_cmp(b)).unwrap_or(0.0)
            }),
        )?;
        self.define_internal_function(
            "sum",
            InternalFunction::new_with_n_or_more_parameters(0, |args| args.iter().sum::<f64>()),
        )?;
        Ok(())
    }

    pub fn define_default_symbols(&mut self) -> Result<(), String> {
        self.add_variable_with_value("pi", std::f64::consts::PI, false)?;
        self.add_variable_with_value("e", std::f64::consts::E, false)?;
        Ok(())
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

    pub(crate) fn add_symbol_from_string(&mut self, string: &str, dry_run: bool) -> Result<String, String> {
        let (sig, def) = string.split_once("=").ok_or("String doesn't contain '='".to_owned())?;
        let sig = Element::parse(sig).ok_or("First formula could not be parsed")?;
        let def = Element::parse(def).ok_or("Second formula could not be parsed")?;

        self.add_symbol_from_sig_and_def(sig, def, dry_run)
    }

    pub fn add_variable_with_value(
        &mut self, name: &str, value: f64, dry_run: bool,
    ) -> Result<String, String> {
        let sig = Element::parse(name).ok_or("First formula could not be parsed")?;
        if !matches!(sig, Element::VariableOrFunction(_) | Element::Variable(_)) {
            return Err("Signature must be a variable".to_owned());
        }
        let def = Element::Number(value);

        self.add_symbol_from_sig_and_def(sig, def, dry_run)
    }

    fn add_symbol_from_sig_and_def(
        &mut self, sig: Element, def: Element, dry_run: bool,
    ) -> Result<String, String> {
        let symbol_name_and_args = SymbolDeclarationData::from_formula(&sig)?;

        if self.internal_function_definitions.contains_key(symbol_name_and_args.get_name()) {
            return Err(format!(
                "Internal function definition with key `{}` already exists",
                symbol_name_and_args.get_name()
            ));
        }
        match self.signatures.add_symbol_from_function_signature_and_definition(
            symbol_name_and_args,
            def.clone(),
            &self.internal_function_definitions,
            dry_run,
        ) {
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
        &self, name: &str, ignore_names: &HashSet<String>,
    ) -> Result<InsertionElement, String> {
        let arguments =
            self.parameter_mappings.get(name).ok_or(format!("Symbol `{name}` not found"))?.clone();
        let mut formula = self.formulas.get(name).ok_or(format!("Symbol `{name}` not found"))?.clone();

        let params_hashset = HashSet::from_iter(arguments.iter().flatten().cloned());
        self.expand_formula(&mut formula, &ignore_names.union(&params_hashset).cloned().collect())?;

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
    store.add_symbol_from_string("f(i)=i^2", false).unwrap();
    store.add_symbol_from_string("g(x)=f(x+1)", false).unwrap();
    store.add_symbol_from_string("h(x)=g(x)-3", false).unwrap();
    let insert = store.get_insertion_element_expanded("h", &HashSet::new()).unwrap();
    dbg!(insert);
}
#[test]
pub fn test_insert_formula() {
    use crate::formula_short::{plus, var};
    let mut store = FormulaStore::new_empty();
    store.add_symbol_from_string("fun(f,x,y)=f(x,y)", false).unwrap();
    store.add_symbol_from_string("add(x,y)=x+y", false).unwrap();
    store.add_symbol_from_string("fun2(x,y)=fun(add, x, y)", false).unwrap();
    let insert = store.get_insertion_element_expanded("fun2", &HashSet::new()).unwrap();
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
                return Err(
                    "The function used in the formula and the supplied function have different parameter counts"
                        .to_owned(),
                );
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
    assert_eq!(store.add_symbol_from_string("f=123", false), Ok("f".to_owned()));
    assert!(matches!(store.add_symbol_from_string("1=1", false), Err(_)));
    assert!(matches!(store.add_symbol_from_string("f=1", false), Err(_)));
    assert!(matches!(store.add_symbol_from_string("x", false), Err(_)));

    assert!(matches!(store.add_symbol_from_string("g(l)=x^2", false), Err(_)));
    assert_eq!(store.add_symbol_from_string("g(g)=g^2", false), Ok("g".to_owned()));
    assert!(matches!(store.add_symbol_from_string("g=2", false), Err(_)));

    assert_eq!(store.add_symbol_from_string("f2(f)=f*3", false), Ok("f2".to_owned()));
    assert!(matches!(store.add_symbol_from_string("f3=f2()", false), Err(_)));

    let result = store.eval("f");
    assert_eq!(result, Ok(123.0));
    assert!(matches!(store.eval("f()"), Err(_)));
    assert!(matches!(store.eval("g()"), Err(_)));
    assert_eq!(store.eval("g(2)"), Ok(4.0));
    assert_eq!(store.eval("f2(2)"), Ok(6.0));

    assert_eq!(store.add_symbol_from_string("add(a,b)=a+b", false), Ok("add".to_owned()));
    assert_eq!(store.add_symbol_from_string("mul(a,b)=a*b", false), Ok("mul".to_owned()));
    assert_eq!(store.add_symbol_from_string("div(a,b)=a/b", false), Ok("div".to_owned()));
    assert!(matches!(store.eval("add(1,2)"), Ok(3.0)));
    assert_eq!(store.add_symbol_from_string("run(a, b, fun)=fun(a, b)", false), Ok("run".to_owned()));
    assert_eq!(store.eval("run(1, 2, add)"), Ok(3.0));
    assert_eq!(store.eval("run(1, 2, mul)"), Ok(2.0));
    assert_eq!(store.eval("run(1, 2, div)"), Ok(0.5));
}
