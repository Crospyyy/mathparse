use crate::calculation::expression_values::{CustomFunction, FunctionExpression};
use crate::parsing::signature::{
	OptionalFunctionDeclarationArguments, ParamCount, Signature, Signatures, SymbolDeclarationData,
};
use crate::{Benchmark, Element, ExpressionFunType, ExpressionNumType, Number, NumberContext, debug_print};
use crate::{create_default_context, only_in_debug};
use anyhow::{Result, anyhow};
use astro_float::ctx::Context;
use std::collections::{HashMap, HashSet};

pub struct FormulaStore {
	symbols: HashMap<String, Symbol>,
}

#[derive(Clone, Debug)]
pub struct Symbol {
	signature: Signature,
	params: Option<Vec<String>>,
	/// This is the original unoptimized formula
	formula: Element,
	/// This is the optimized and reduced formula, which is used for evaluation
	optimized_formula: Element,
}

#[derive(Debug)]
pub struct NamedSymbol {
	name: String,
	symbol: Symbol,
}

#[derive(Debug)]
pub struct InsertionElement {
	name: String,
	parameters: Option<Vec<String>>,
	formula: Element,
}

// public functions
impl FormulaStore {
	pub fn get_symbols_sorted(&'_ self) -> Vec<(&String, &Symbol)> {
		let mut vec = self.symbols.iter().collect::<Vec<_>>();
		vec.sort_by(|a, b| a.0.cmp(b.0));
		vec
	}

	pub fn define_default_symbols(&mut self) -> Result<()> {
		macro_rules! define_functions {
            ($($op:ident),+) => {
                $(self.add_expression_fun(&stringify!($op).to_lowercase(), ExpressionFunType::$op, false)?);+
            }
        }

		define_functions!(
			Sin, Asin, Cos, Acos, Tan, Atan, Floor, Ceil, Round, Abs, Rem, Log2, Log10, Ln, Fac
		);

		self.add_expression_fun(
			// todo add these as native expression functions
			"avg",
			ExpressionFunType::Custom(CustomFunction::new(
				"avg",
				ParamCount::AtLeast(1),
				FunctionExpression::multiple_arguments(|args: Vec<Number>, ctx: &mut Context| {
					if let Some(average) = Number::average(&args, ctx) {
						average
					} else {
						eprintln!("Somehow average was called with zero numbers");
						Number::nan(None)
					}
				}),
			)),
			false,
		)?;
		self.add_expression_fun(
			"median",
			CustomFunction::multiple_arguments(
				"median",
				ParamCount::AtLeast(1),
				|args: Vec<Number>, ctx: &mut Context| {
					if let Some(median) = Number::median(&args, ctx) {
						median
					} else {
						eprintln!("Somehow median was called with zero numbers");
						Number::nan(None)
					}
				},
			)
			.into(),
			false,
		)?;
		self.add_expression_fun(
			"max",
			CustomFunction::multiple_arguments(
				"max",
				ParamCount::AtLeast(1),
				|args: Vec<Number>, ctx: &mut Context| {
					if let Some(max) = Number::max_of_several(&args, ctx) {
						max
					} else {
						eprintln!("Somehow max was called with zero numbers");
						Number::nan(None)
					}
				},
			)
			.into(),
			false,
		)?;
		self.add_expression_fun(
			"min",
			CustomFunction::multiple_arguments(
				"min",
				ParamCount::AtLeast(1),
				|args: Vec<Number>, ctx: &mut Context| {
					if let Some(min) = Number::min_of_several(&args, ctx) {
						min
					} else {
						eprintln!("Somehow min was called with zero numbers");
						Number::nan(None)
					}
				},
			)
			.into(),
			false,
		)?;
		self.add_expression_fun(
			"sum",
			CustomFunction::multiple_arguments(
				"sum",
				ParamCount::AtLeast(0),
				|args: Vec<Number>, ctx: &mut Context| Number::sum(&args, ctx),
			)
			.into(),
			false,
		)?;

		self.add_expression_var("pi", ExpressionNumType::Pi, false)?;
		self.add_expression_var("e", ExpressionNumType::E, false)?;

		self.add_symbol_from_string("deg(rad)=rad/pi*180", false)?;
		self.add_symbol_from_string("rad(deg)=deg/180*pi", false)?;
		self.add_symbol_from_string("sqrt(x)=x^(1/2)", false)?;
		self.add_symbol_from_string("binomial(n, k) = fac(n) / (fac(n - k) * fac(k))", false)?;

		Ok(())
	}

	pub fn new_empty() -> Self {
		FormulaStore { symbols: HashMap::new() }
	}

	pub fn new_with_default_symbols() -> Self {
		let mut store = Self::new_empty();
		store.define_default_symbols().expect("Failed to define default symbols");
		store
	}

	pub fn add_symbol_from_string(&mut self, string: &str, dry_run: bool) -> Result<NamedSymbol> {
		let (sig, def) = string.split_once("=").ok_or(anyhow!("String doesn't contain '='"))?;
		let sig = Element::parse(sig).map_err(|err| anyhow!("First formula could not be parsed: {err}"))?;
		let def = Element::parse(def).map_err(|err| anyhow!("Second formula could not be parsed: {err}"))?;
		self.add_symbol_from_sig_and_def(sig, def, dry_run)
	}

	// todo check whether this is needed
	#[allow(unused)]
	pub(crate) fn get_insertion_element(&self, name: &str) -> Option<InsertionElement> {
		let symbol = self.symbols.get(name)?;
		Some(InsertionElement {
			name: name.to_string(),
			parameters: symbol.params.clone(),
			formula: symbol.formula.clone(),
		})
	}

	pub(crate) fn get_insertion_element_expanded(
		&self, name: &str, _ignore_names: &HashSet<String>,
	) -> Result<InsertionElement> {
		let symbol = self.symbols.get(name).ok_or(anyhow!("Symbol `{name}` not found"))?;

		let formula = symbol.optimized_formula.clone();
		// let params_hashset = HashSet::from_iter(symbol.params.iter().flatten().cloned());

		// self.expand_formula(&mut formula, &ignore_names.union(&params_hashset).cloned().collect())?;

		Ok(InsertionElement { name: name.to_string(), parameters: symbol.params.clone(), formula })
	}

	pub fn get_symbol(&self, name: &str) -> Option<&Symbol> {
		self.symbols.get(name)
	}
}

// private functions
impl FormulaStore {
	fn resolve_formula_and_refine_call_signature(
		symbol_name_and_args: &mut OptionalFunctionDeclarationArguments, content: &Element,
		defined_signatures: &Signatures,
	) -> Result<()> {
		// Initialize signatures and
		// add the already defined functions with variable argument count
		let mut required_signatures: Signatures = defined_signatures.clone();
		required_signatures.retain(|_, signature| matches!(signature, Signature::FunctionNOrMoreParams(_)));
		required_signatures.add_all_undefined_symbols_of_formula(content);

		Signatures::refine_signature_and_undefined(
			symbol_name_and_args,
			&mut required_signatures,
			content,
			defined_signatures,
		)?;

		if !required_signatures.is_empty() {
			return Err(anyhow!(
				"The formula requires the following elements to be defined: {:?}",
				required_signatures.0
			));
		}
		Ok(())
	}

	fn check_symbol_name_availability(&self, name: &String) -> Result<()> {
		if self.symbols.contains_key(name) {
			return Err(anyhow!("The formula {} is already defined", name));
		}
		Ok(())
	}

	fn add_symbol_new(&mut self, name: &str, symbol: Symbol, dry_run: bool) -> Result<()> {
		if self.symbols.contains_key(name) {
			return Err(anyhow!("The formula {} is already defined", name));
		}
		if !dry_run {
			self.symbols.insert(name.to_string(), symbol);
		}
		Ok(())
	}

	fn resolve_new_symbol(
		&self, mut opt_func_args: OptionalFunctionDeclarationArguments, content: Element,
	) -> Result<Symbol> {
		let defined_signatures: Signatures =
			self.symbols.iter().map(|(name, symbol)| (name.clone(), symbol.signature.clone())).collect();

		Self::resolve_formula_and_refine_call_signature(&mut opt_func_args, &content, &defined_signatures)?;
		let mut optimized_and_expanded = content.clone();
		let flatten = opt_func_args.iter().flat_map(|x| x.names.iter()).cloned().collect();
		self.expand_and_optimize(
			&mut optimized_and_expanded,
			&mut Benchmark::new("Expansion and optimization"),
			&flatten,
		)?;
		Ok(Symbol::create_from(opt_func_args, content, optimized_and_expanded))
	}

	fn add_symbol_from_sig_and_def(
		&mut self, sig: Element, def: Element, dry_run: bool,
	) -> Result<NamedSymbol> {
		let symbol_name_and_args = SymbolDeclarationData::from_formula(&sig)?;
		self.check_symbol_name_availability(symbol_name_and_args.get_name())?;

		let symbol = self.resolve_new_symbol(symbol_name_and_args.function_args, def)?;

		debug_print!(
			"New formula definition: {}",
			symbol.get_full_string(&symbol_name_and_args.name, &mut create_default_context(), true)
		);
		self.add_symbol_new(&symbol_name_and_args.name, symbol.clone(), dry_run)?;

		Ok(NamedSymbol::new(symbol_name_and_args.name, symbol))
	}

	fn add_expression_var(
		&mut self, name: impl ToString, expr_value: ExpressionNumType, dry_run: bool,
	) -> Result<()> {
		let parameter_names = None;
		let signature = Signature::Number;
		let formula = Element::NumberWithExpression { expr_value };
		self.add_symbol_new(
			&name.to_string(),
			Symbol::new(signature, parameter_names, formula.clone(), formula),
			dry_run,
		)
	}

	fn add_expression_fun(
		&mut self, name: impl ToString, expr_value: ExpressionFunType, dry_run: bool,
	) -> Result<()> {
		let params = Some(vec![]); // todo add param names
		let signature = match expr_value.get_param_count() {
			ParamCount::Exactly(n) => Signature::Function(vec![Signature::Number; n]),
			ParamCount::AtLeast(n) => Signature::FunctionNOrMoreParams(n),
		};
		let element = Element::FunctionWithExpression { arguments: vec![], expr_value };
		self.add_symbol_new(
			&name.to_string(),
			Symbol::new(signature, params, element.clone(), element),
			dry_run,
		)
	}
}

impl Symbol {
	pub(crate) fn create_from(
		opt_func_args: OptionalFunctionDeclarationArguments, content: Element, optimized: Element,
	) -> Self {
		Symbol::new(
			opt_func_args
				.as_ref()
				.map(|args| Signature::Function(args.get_signatures_in_right_order()))
				.unwrap_or(Signature::Number),
			opt_func_args.as_ref().map(|b| b.names.clone()),
			content,
			optimized,
		)
	}

	pub(crate) fn new(
		signature: Signature, params: Option<Vec<String>>, formula: Element, optimized: Element,
	) -> Self {
		Self { signature, params, formula, optimized_formula: optimized }
	}

	pub fn signature(&self) -> &Signature {
		&self.signature
	}

	pub fn params(&self) -> Option<&Vec<String>> {
		self.params.as_ref()
	}

	pub fn formula(&self) -> &Element {
		&self.formula
	}
	
	/// Get the call signature string like `fun(a, b)` or `var_xy`
	pub fn get_signature_string(&self, name: &str) -> String {
		match self.signature {
			Signature::NumberOrFunction => name.to_string(),
			Signature::Number => name.to_string(),
			Signature::Function(_) => {
				format!("{}({})", name, self.params.as_ref().map_or("".to_owned(), |p| p.join(", ")))
			},
			Signature::FunctionNOrMoreParams(n) => format!("{}({}..)", name, n),
			Signature::Conflicting => "Conflicting".to_owned(),
		}
	}

	pub fn get_full_string(
		&self, name: &str, ctx: &mut NumberContext, show_optimized_formula: bool,
	) -> String {
		format!(
			"{} = {}",
			self.get_signature_string(name),
			if show_optimized_formula { &self.optimized_formula } else { &self.formula }.get_string(ctx)
		)
	}
}

impl NamedSymbol {
	fn new(name: String, symbol: Symbol) -> NamedSymbol {
		Self { name, symbol }
	}

	pub fn get_signature_string(&self) -> String {
		self.symbol.get_signature_string(&self.name)
	}

	pub fn name(&self) -> &String {
		&self.name
	}

	pub fn symbol(&self) -> &Symbol {
		&self.symbol
	}

	pub fn get_full_string(&self, ctx: &mut NumberContext, show_optimized_formula: bool) -> String {
		self.symbol.get_full_string(&self.name, ctx, show_optimized_formula)
	}
}

impl InsertionElement {
	pub fn insert_param_values(&self, param_values: Vec<Element>) -> Result<Element> {
		if let Some(insert_args) = &self.parameters {
			if let Element::FunctionWithExpression { expr_value, .. } = &self.formula
				&& insert_args.is_empty()
			{
				if !expr_value.get_param_count().number_would_be_valid(param_values.len()) {
					return Err(anyhow!(
						"The function `{}` expects parameters, that match {:?}, but {} parameters were provided",
						self.name,
						expr_value.get_param_count(),
						param_values.len()
					));
				}
				return Ok(Element::FunctionWithExpression {
					arguments: param_values,
					expr_value: expr_value.clone(),
				});
			}
			let self_arguments = param_values;
			if insert_args.len() != self_arguments.len() {
				dbg!(insert_args);
				dbg!(self_arguments);
				return Err(anyhow!(
					"The function used in the formula and the supplied function have different parameter counts"
				));
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
		Err(anyhow!("No arguments provided for insertion"))
	}
}

impl Element {
	pub(crate) fn insert_symbol(&mut self, insert: &InsertionElement) -> Result<()> {
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
						return Err(anyhow!(
							"Insertion element and formula don't match (self: {:?}, insert: {:?})",
							self,
							insert
						));
					},
				},
				(Element::Variable(_), None, _) => {
					*self = insert.formula.clone();
				},
				(Element::VariableOrFunction(..), params, _) => {
					if params.is_some() {
						println!("Skipping this because there is no call yet");
					} else {
						// insertion element is either a variable or also a variable_or_function
						*self = insert.formula.clone();
					}
				},

				(..) => {
					return Err(anyhow!(
						"Insertion element and formula don't match (self: {:?}, insert: {:?})",
						self,
						insert
					));
				},
			}
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

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::Benchmark;
	use crate::formula_short::{fun_expr, mul, num, var};
	use crate::outer_store_interaction::RunPrecision;
	use macros::formula_matches;

	#[test]
	fn test_float_consts() {
		use crate::calculation::create_default_context;
		use astro_float::BigFloat;
		use astro_float::expr;

		let ctx = &mut create_default_context();
		assert!(ctx.const_pi().inexact());
		assert!(ctx.const_e().inexact());
		assert!(!BigFloat::nan(None).inexact());
		assert!(!expr!(sqrt(16), &mut *ctx).inexact());
		assert!(expr!(pow(16, 0.5), &mut *ctx).inexact());
	}

	macro_rules! check_add {
		($store:expr,$string:expr, $expected:expr) => {
			assert_eq!(
				$store.add_symbol_from_string($string, false).map(|s| s.name).ok(),
				Some($expected.to_owned())
			);
		};
	}

	#[test]
	fn test_add_symbols() {
		let mut all = FormulaStore::new_empty();
		check_add!(all, "fun(a,b)=a+b", "fun");
		assert!(all.add_symbol_from_string("fun(a,b)=a+b", false).is_err());
		println!("{:?}", all.get_symbols_sorted());
		check_add!(all, "fun2(a,b,c)=fun(a,b)+c", "fun2");
		println!("{:?}", all.get_symbols_sorted());
		check_add!(all, "fun3(some_fun)=fun(1,2)+some_fun(3)", "fun3");
		println!("{:?}", all.get_symbols_sorted());
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
		assert_eq!(insert.parameters, Some(vec!["x".to_owned(), "y".to_owned()]));
		assert_eq!(insert.formula, plus([var("x"), var("y")]));
		dbg!(insert);
	}

	#[test]
	fn test_insert_symbols() {
		println!("### Test inserting symbols ###");

		let fun = Element::parse("x+y").unwrap();

		let mut formula = Element::parse("f(12, f(1, 2))").unwrap();
		formula
			.insert_symbol(&InsertionElement {
				name: "f".to_owned(),
				parameters: Some(vec!["x".to_owned(), "y".to_owned()]),
				formula: fun,
			})
			.unwrap();
		assert!(formula_matches!(formula, plus(num(12), plus(num(1), num(2)))));
		let mut formula = Element::parse("f(3)").unwrap();
		formula
			.insert_symbol(&InsertionElement {
				name: "f".to_owned(),
				parameters: Some(vec!["x".to_owned()]),
				formula: fun_expr(ExpressionFunType::Abs, [mul([var("x"), num(2)])]),
			})
			.unwrap();
		if let Element::FunctionWithExpression { expr_value, arguments } = &formula {
			assert_eq!(arguments.len(), 1);
			assert_eq!(arguments[0], mul([num(3), num(2)]));
			assert_eq!(expr_value, &ExpressionFunType::Abs);
		} else {
			panic!("Expected function with expression");
		}

		let mut formula = Element::parse("f(3)").unwrap();
		formula
			.insert_symbol(&InsertionElement {
				name: "f".to_owned(),
				parameters: Some(vec![]),
				formula: fun_expr(ExpressionFunType::Abs, [mul([var("x"), num(2)])]),
			})
			.unwrap();
		if let Element::FunctionWithExpression { expr_value, arguments } = &formula {
			assert_eq!(arguments.len(), 1);
			assert_eq!(arguments[0], num(3));
			assert_eq!(expr_value, &ExpressionFunType::Abs);
		} else {
			panic!("Expected function with expression");
		}
	}

	#[test]
	fn test_storing() {
		println!("### Test storing formulas ###");

		let mut store = FormulaStore::new_empty();

		check_add!(store, "f=123", "f");
		assert!(store.add_symbol_from_string("1=1", false).is_err());
		assert!(store.add_symbol_from_string("f=1", false).is_err());
		assert!(store.add_symbol_from_string("x", false).is_err());

		assert!(store.add_symbol_from_string("g(l)=x^2", false).is_err());
		check_add!(store, "g(g)=g^2", "g");
		assert!(store.add_symbol_from_string("g=2", false).is_err());

		check_add!(store, "f2(f)=f*3", "f2");
		assert!(store.add_symbol_from_string("f3=f2()", false).is_err());

		store.quick_eval("f", 123);
		assert!(store.eval_new("f()", RunPrecision::default(), &mut Benchmark::new("Evaluation")).is_err());
		assert!(store.eval_new("g()", RunPrecision::default(), &mut Benchmark::new("Evaluation")).is_err());
		store.quick_eval("g(2)", 4);
		store.quick_eval("f2(2)", 6);

		check_add!(store, "add(a,b)=a+b", "add");
		check_add!(store, "mul(a,b)=a*b", "mul");
		check_add!(store, "div(a,b)=a/b", "div");
		store.quick_eval("add(1,2)", 3);

		check_add!(store, "run(a, b, fun)=fun(a, b)", "run");
		store.quick_eval("run(1, 2, add)", 3);
		store.quick_eval("run(1, 2, mul)", 2);
		store.quick_eval2("run(1, 2, div)", "0.5");
	}
}
