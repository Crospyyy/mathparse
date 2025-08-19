use crate::parsing::signature::{ParamCount, Signature, Signatures, SymbolDeclarationData};
use crate::{
    Element, ExprValue, ExpressionFunType, ExpressionNumType, FunctionExpression, Number, NumberContext,
};
use astro_float::ctx::Context;
use std::collections::{HashMap, HashSet};

pub struct FormulaStore {
    signatures: Signatures,
    formulas: HashMap<String, Element>,
    parameter_mappings: HashMap<String, Option<Vec<String>>>,
}

#[test]
fn test_float_consts() {
    use crate::operations::create_default_context;
    use astro_float::BigFloat;
    use astro_float::expr;

    let ctx = &mut create_default_context();
    assert_eq!(ctx.const_pi().inexact(), true);
    assert_eq!(ctx.const_e().inexact(), true);
    assert_eq!(BigFloat::nan(None).inexact(), false);
    assert_eq!(expr!(sqrt(16), &mut *ctx).inexact(), false);
    assert_eq!(expr!(pow(16, 0.5), &mut *ctx).inexact(), true);
}

pub struct Symbol<'a> {
    name: &'a str,
    signature: &'a Signature,
    params: Option<&'a Vec<String>>,
    formula: &'a Element,
}

impl Symbol<'_> {
    pub fn name(&self) -> &str {
        self.name
    }

    pub fn signature(&self) -> &Signature {
        self.signature
    }

    pub fn params(&self) -> Option<&Vec<String>> {
        self.params
    }

    pub fn formula(&self) -> &Element {
        self.formula
    }

    pub fn get_signature_string(&self) -> String {
        match self.signature {
            Signature::NumberOrFunction => self.name.to_string(),
            Signature::Number => self.name.to_string(),
            Signature::Function(_) => {
                format!("{}({})", self.name, self.params.map_or("".to_string(), |p| p.join(", ")))
            },
            Signature::FunctionNOrMoreParams(n) => format!("{}({}..)", self.name, n),
            Signature::Conflicting => "Conflicting".to_string(),
        }
    }

    pub fn get_full_string(&self, ctx: &mut NumberContext) -> String {
        format!("{} = {}", self.get_signature_string(), self.formula.get_string(ctx))
    }
}

impl FormulaStore {
    pub fn get_symbols(&self) -> Vec<Symbol> {
        self.parameter_mappings
            .iter()
            .map(|(name, params)| Symbol {
                name,
                signature: self.signatures.get(name).unwrap(),
                params: params.as_ref(),
                formula: self.formulas.get(name).unwrap(),
            })
            .collect::<Vec<_>>()
    }

    pub fn define_default_symbols(&mut self) -> Result<(), String> {
        macro_rules! define_fun_single_arg {
            ($($op:ident),+) => {
                $(self.add_expression_fun_single_arg(stringify!($op), Number::$op, "x", stringify!($op).into())?);+
            }
        }

        define_fun_single_arg!(sin, cos, tan, asin, acos, atan, abs, log2, log10, ln, floor, ceil, round);

        self.add_expression_fun_multiple_args(
            "avg",
            ParamCount::AtLeast(1),
            |ctx: &mut Context, args: Vec<Number>| {
                if let Some(average) = Number::average(&args, ctx) {
                    average
                } else {
                    eprintln!("Somehow average was called with zero numbers");
                    Number::nan(None)
                }
            },
            "avg".into(),
        )?;
        self.add_expression_fun_multiple_args(
            "median",
            ParamCount::AtLeast(1),
            |ctx: &mut Context, args: Vec<Number>| {
                if let Some(median) = Number::median(&args, ctx) {
                    median
                } else {
                    eprintln!("Somehow median was called with zero numbers");
                    Number::nan(None)
                }
            },
            "median".into(),
        )?;
        self.add_expression_fun_multiple_args(
            "max",
            ParamCount::AtLeast(1),
            |ctx: &mut Context, args: Vec<Number>| {
                if let Some(max) = Number::max_of_several(&args, ctx) {
                    max
                } else {
                    eprintln!("Somehow max was called with zero numbers");
                    Number::nan(None)
                }
            },
            "max".into(),
        )?;
        self.add_expression_fun_multiple_args(
            "min",
            ParamCount::AtLeast(1),
            |ctx: &mut Context, args: Vec<Number>| {
                if let Some(min) = Number::min_of_several(&args, ctx) {
                    min
                } else {
                    eprintln!("Somehow min was called with zero numbers");
                    Number::nan(None)
                }
            },
            "min".into(),
        )?;
        self.add_expression_fun_multiple_args(
            "sum",
            ParamCount::AtLeast(0),
            |ctx: &mut Context, args: Vec<Number>| Number::sum(&args, ctx),
            "sum".into(),
        )?;
        self.add_expression_fun_multiple_args(
            "rem",
            ExpressionFunType::Rem.get_param_count(),
            ExpressionFunType::Rem.get_function_multiple_args().unwrap(),
            "rem".into(),
        )?;

        self.add_expression_var("pi", ExpressionNumType::Pi.get_function(), "pi".into())?;
        self.add_expression_var("e", ExpressionNumType::E.get_function(), "e".into())?;
        self.add_symbol_from_string("deg(rad)=rad/pi*180", false)?;
        self.add_symbol_from_string("rad(deg)=deg/180*pi", false)?;
        self.add_symbol_from_string("sqrt(x)=x^(1/2)", false)?;

        Ok(())
    }

    pub fn new_empty() -> Self {
        FormulaStore {
            signatures: Signatures::new_empty(),
            formulas: HashMap::new(),
            parameter_mappings: HashMap::new(),
        }
    }

    pub fn add_symbol_from_string(&mut self, string: &str, dry_run: bool) -> Result<String, String> {
        let (sig, def) = string.split_once("=").ok_or("String doesn't contain '='".to_owned())?;
        let sig = Element::parse(sig).map_err(|err| format!("First formula could not be parsed: {err}"))?;
        let def = Element::parse(def).map_err(|err| format!("Second formula could not be parsed: {err}"))?;

        self.add_symbol_from_sig_and_def(sig, def, dry_run)
    }

    fn add_symbol_from_sig_and_def(
        &mut self, sig: Element, def: Element, dry_run: bool,
    ) -> Result<String, String> {
        let symbol_name_and_args = SymbolDeclarationData::from_formula(&sig)?;

        match self.signatures.add_symbol_from_function_signature_and_definition(
            symbol_name_and_args,
            def.clone(),
            dry_run,
        ) {
            Ok((name, arg_names)) => {
                if !dry_run {
                    self.formulas.insert(name.clone(), def);
                    self.parameter_mappings.insert(name.clone(), arg_names);
                }

                Ok(name)
            },
            Err(err) => Err(format!("Could not add symbol: {}", err)),
        }
    }

    pub fn add_variable_with_value(
        &mut self, name: &str, value: impl ToString, dry_run: bool,
    ) -> Result<(), String> {
        let value = value.to_string();
        let sig = Element::parse(name).map_err(|err| format!("First formula could not be parsed: {err}"))?;
        if !matches!(sig, Element::VariableOrFunction(_) | Element::Variable(_)) {
            return Err("Signature must be a variable".to_owned());
        }
        let number = Number::from_string(&value).ok_or(format!("Invalid number: {}", value))?;
        let def = Element::Number(number);

        self.add_symbol_from_sig_and_def(sig, def, dry_run)?;
        Ok(())
    }

    pub fn add_expression_var(
        &mut self, name: impl ToString, expression: fn(&mut Context) -> Number,
        debug_name: ExprValue<ExpressionNumType>,
    ) -> Result<(), String> {
        let name = name.to_string();
        if self.formulas.contains_key(&name) {
            return Err(format!("Formula definition with key `{}` already exists", name));
        }
        self.parameter_mappings.insert(name.clone(), None);
        self.signatures.insert(name.clone(), Signature::Number);
        self.formulas.insert(name, Element::NumberWithExpression { fun: expression, expr_value: debug_name });
        Ok(())
    }

    pub fn add_expression_fun_multiple_args(
        &mut self, name: impl ToString, param_count: ParamCount,
        expression: fn(&mut Context, Vec<Number>) -> Number, debug_name: ExprValue<ExpressionFunType>,
    ) -> Result<(), String> {
        let name = name.to_string();
        if self.formulas.contains_key(&name) {
            return Err(format!("Formula definition with key `{}` already exists", name));
        }
        self.parameter_mappings.insert(name.clone(), Some(vec![])); // add a mock parameter list
        self.signatures.insert(
            name.clone(),
            match param_count {
                ParamCount::Exactly(n) => Signature::Function(vec![Signature::Number; n]),
                ParamCount::AtLeast(n) => Signature::FunctionNOrMoreParams(n),
            },
        );
        self.formulas.insert(
            name,
            Element::FunctionWithExpression {
                arguments: vec![],
                param_count,
                expression: FunctionExpression::MultipleArguments(expression),
                expr_value: debug_name,
            },
        );
        Ok(())
    }
    pub fn add_expression_fun_single_arg(
        &mut self, name: impl ToString, expression: fn(&Number, &mut Context) -> Number, param_name: &str,
        debug_name: ExprValue<ExpressionFunType>,
    ) -> Result<(), String> {
        let name = name.to_string();
        if self.formulas.contains_key(&name) {
            return Err(format!("Formula definition with key `{}` already exists", name));
        }
        self.parameter_mappings.insert(name.clone(), Some(vec![param_name.to_string()])); // add a mock parameter list
        self.signatures.insert(name.clone(), Signature::Function(vec![Signature::Number]));
        self.formulas.insert(
            name,
            Element::FunctionWithExpression {
                arguments: vec![Element::Variable(param_name.to_string())],
                param_count: ParamCount::Exactly(1),
                expression: FunctionExpression::SingleArgument(expression),
                expr_value: debug_name,
            },
        );
        Ok(())
    }

    pub(crate) fn get_insertion_element(&self, name: &str) -> Option<InsertionElement> {
        Some(InsertionElement {
            name: name.to_string(),
            parameters: self.parameter_mappings.get(name)?.clone(),
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

        Ok(InsertionElement { name: name.to_string(), parameters: arguments, formula })
    }

    pub fn get_signature(&self, name: &str) -> Option<&Signature> {
        self.signatures.get(name)
    }

    #[cfg(test)]
    pub(crate) fn get_signatures(&self) -> &Signatures {
        &self.signatures
    }
}

#[test]
fn test_add_symbols() {
    let mut all = FormulaStore::new_empty();
    assert_eq!(all.add_symbol_from_string("fun(a,b)=a+b", false), Ok("fun".to_owned()));
    assert!(matches!(all.add_symbol_from_string("fun(a,b)=a+b", false), Err(_)));
    println!("{:?}", all.get_signatures());
    assert_eq!(all.add_symbol_from_string("fun2(a,b,c)=fun(a,b)+c", false), Ok("fun2".to_owned()));
    println!("{:?}", all.get_signatures());
    let result = all.add_symbol_from_string("fun3(some_fun)=fun(1,2)+some_fun(3)", false);
    println!("{:?}", result);
    assert_eq!(result, Ok("fun3".to_owned()));
    println!("{:?}", all.get_signatures());
}

#[test]
fn test_get_insertion_element_expanded() {
    let mut store = FormulaStore::new_empty();
    store.add_symbol_from_string("f(i)=i^2", false).unwrap();
    store.add_symbol_from_string("g(x)=f(x+1)", false).unwrap();
    store.add_symbol_from_string("h(x)=g(x)-3", false).unwrap();
    let insert = store.get_insertion_element_expanded("h", &HashSet::new()).unwrap();
    dbg!(insert);
}
#[test]
fn test_insert_formula() {
    use crate::formula_short::{plus, var};
    let mut store = FormulaStore::new_empty();
    store.add_symbol_from_string("fun(f,x,y)=f(x,y)", false).unwrap();
    store.add_symbol_from_string("add(x,y)=x+y", false).unwrap();
    store.add_symbol_from_string("fun2(x,y)=fun(add, x, y)", false).unwrap();
    let insert = store.get_insertion_element_expanded("fun2", &HashSet::new()).unwrap();
    assert_eq!(insert.name, "fun2");
    assert_eq!(insert.parameters, Some(vec!["x".to_string(), "y".to_string()]));
    assert_eq!(insert.formula, plus([var("x"), var("y")]));
    dbg!(insert);
}

#[derive(Debug)]
pub struct InsertionElement {
    name: String,
    parameters: Option<Vec<String>>,
    formula: Element,
}

impl InsertionElement {
    pub fn insert_param_values(&self, param_values: Vec<Element>) -> Result<Element, String> {
        if let Some(insert_args) = &self.parameters {
            if let Element::FunctionWithExpression {
                expression, param_count, expr_value: debug_name, ..
            } = &self.formula
            {
                if !param_count.number_would_be_valid(param_values.len()) {
                    return Err(format!(
                        "The function `{}` expects parameters, that match {:?}, but {} parameters were provided",
                        self.name,
                        param_count,
                        param_values.len()
                    ));
                }
                return Ok(Element::FunctionWithExpression {
                    arguments: param_values,
                    param_count: *param_count,
                    expression: expression.clone(),
                    expr_value: debug_name.clone(),
                });
            }
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
                    parameters: None,
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
            | Element::Function { arguments: elements, .. }
            | Element::FunctionWithExpression { arguments: elements, .. } => {
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
            | Element::NumberWithExpression { .. }
            | Element::Variable(_)
            | Element::VariableOrFunction(_)
            | Element::String(_) => {},
        }
        if self.get_name().is_some_and(|n| n == insert.name) {
            // this is going to be the new logic
            match (&mut *self, &insert.parameters, &insert.formula) {
                (
                    Element::Function { name: self_name, arguments: self_arguments },
                    insert_params,
                    insert_formula,
                ) => match (insert_params, insert_formula) {
                    (None, Element::VariableOrFunction(new_name)) => {
                        *self_name = new_name.clone();
                    },
                    (Some(_), _) => {
                        *self = insert.insert_param_values(self_arguments.clone())?;
                    },
                    (None, Element::FunctionWithExpression { .. }) => {
                        *self = insert.insert_param_values(self_arguments.clone())?;
                    },
                    (..) => {
                        return Err(format!(
                            "Insertion element and formula don't match (self: {:?}, insert: {:?})",
                            self, insert
                        ));
                    },
                },
                (Element::Variable(_), None, _) => {
                    *self = insert.formula.clone();
                },
                (Element::VariableOrFunction(name), params, _) => {
                    if params.is_some() {
                        println!("Skipping this because there is no call yet");
                    } else {
                        // insertion element is either a variable or also a variable_or_function
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
            | Element::VariableOrFunction(_)
            | Element::NumberWithExpression { .. }
            | Element::Function { .. } => {},
            Element::Plus(elements)
            | Element::Multiply(elements)
            | Element::FunctionWithExpression { arguments: elements, .. } => {
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
        }
        Ok(())
    }
}

#[test]
fn test_insert_symbols() {
    println!("### Test inserting symbols ###");

    let fun = Element::parse("x+y").unwrap();

    let mut formula = Element::parse("f(12, f(1, 2))").unwrap();
    println!("{}", formula.get_debug_string());
    formula
        .insert_symbol(&InsertionElement {
            name: "f".to_owned(),
            parameters: Some(vec!["x".to_owned(), "y".to_owned()]),
            formula: fun,
        })
        .unwrap();
    println!("{}", formula.get_debug_string());
}

#[test]
fn test_storing() {
    use crate::operations::create_default_context;

    println!("### Test storing formulas ###");

    let mut store = FormulaStore::new_empty();
    let mut ctx = create_default_context();
    assert_eq!(store.add_symbol_from_string("f=123", false), Ok("f".to_owned()));
    assert!(matches!(store.add_symbol_from_string("1=1", false), Err(_)));
    assert!(matches!(store.add_symbol_from_string("f=1", false), Err(_)));
    assert!(matches!(store.add_symbol_from_string("x", false), Err(_)));

    assert!(matches!(store.add_symbol_from_string("g(l)=x^2", false), Err(_)));
    assert_eq!(store.add_symbol_from_string("g(g)=g^2", false), Ok("g".to_owned()));
    assert!(matches!(store.add_symbol_from_string("g=2", false), Err(_)));

    assert_eq!(store.add_symbol_from_string("f2(f)=f*3", false), Ok("f2".to_owned()));
    assert!(matches!(store.add_symbol_from_string("f3=f2()", false), Err(_)));

    let result = store.eval("f", &mut ctx);
    assert_eq!(result, Ok(123.into()));
    assert!(matches!(store.eval("f()", &mut ctx), Err(_)));
    assert!(matches!(store.eval("g()", &mut ctx), Err(_)));
    assert_eq!(store.eval("g(2)", &mut ctx), Ok(4.into()));
    assert_eq!(store.eval("f2(2)", &mut ctx), Ok(6.into()));

    assert_eq!(store.add_symbol_from_string("add(a,b)=a+b", false), Ok("add".to_owned()));
    assert_eq!(store.add_symbol_from_string("mul(a,b)=a*b", false), Ok("mul".to_owned()));
    assert_eq!(store.add_symbol_from_string("div(a,b)=a/b", false), Ok("div".to_owned()));
    assert_eq!(store.eval("add(1,2)", &mut ctx), Ok(3.into()));
    assert_eq!(store.add_symbol_from_string("run(a, b, fun)=fun(a, b)", false), Ok("run".to_owned()));
    assert_eq!(store.eval("run(1, 2, add)", &mut ctx), Ok(3.into()));
    assert_eq!(store.eval("run(1, 2, mul)", &mut ctx), Ok(2.into()));
    assert_eq!(store.eval("run(1, 2, div)", &mut ctx), Ok(Number::from_string("0.5").unwrap()));
}
