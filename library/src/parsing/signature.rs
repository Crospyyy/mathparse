use crate::ElementParsed;
use anyhow::{Result, anyhow};
use std::cmp::PartialEq;
use std::collections::{HashMap, HashSet};
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, PartialEq)]
pub enum Signature {
	NumberOrFunction,
	Number,
	Function(Vec<Signature>),
	/// This Function only takes numbers as parameters
	FunctionNOrMoreParams(usize),
	Conflicting,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ParamCount {
	Exactly(usize),
	AtLeast(usize),
}

#[derive(Clone, Debug)]
pub struct Signatures(pub(crate) HashMap<String, Signature>);

pub struct SymbolDeclarationData {
	pub(crate) name: String,
	pub(crate) function_args: OptionalFunctionDeclarationArguments,
}

pub(crate) struct OptionalFunctionDeclarationArguments(Option<FunctionDeclarationArguments>);

#[derive(Debug)]
pub(crate) struct FunctionDeclarationArguments {
	pub(crate) names: Vec<String>,
	signatures: Signatures,
}

impl ParamCount {
	pub(crate) fn number_would_be_valid(&self, param_count: usize) -> bool {
		match self {
			ParamCount::Exactly(n) => param_count == *n,
			ParamCount::AtLeast(n) => param_count >= *n,
		}
	}
}

impl Signature {
	fn refine_with(&mut self, new: Self) {
		match (&mut *self, new) {
			(Signature::NumberOrFunction, new) => *self = new,
			(_, Signature::NumberOrFunction) | (Signature::Number, Signature::Number) => {},
			(Signature::Function(args_old), Signature::Function(args_new)) => {
				if args_old.len() != args_new.len() {
					*self = Signature::Conflicting;
					return;
				}
				args_old.iter_mut().zip(args_new).for_each(|(a, b)| {
					a.refine_with(b);
				});
				if args_old.iter().any(|a| matches!(a, Signature::Conflicting)) {
					*self = Signature::Conflicting;
				}
			},
			(Signature::Function(args_old), Signature::FunctionNOrMoreParams(at_least)) => {
				if args_old.len() < at_least {
					*self = Signature::Conflicting;
					return;
				}
				if !args_old.iter().all(|a| matches!(a, Signature::Number | Signature::NumberOrFunction)) {
					*self = Signature::Conflicting;
					return;
				}
				*self = Signature::FunctionNOrMoreParams(at_least)
			},
			(Signature::FunctionNOrMoreParams(at_least), Signature::Function(params)) => {
				if params.len() < *at_least
					|| !params.iter().all(|p| matches!(p, Signature::Number | Signature::NumberOrFunction))
				{
					*self = Signature::Conflicting;
				}
			},
			(
				Signature::FunctionNOrMoreParams(at_least_old),
				Signature::FunctionNOrMoreParams(at_least_new),
			) => {
				if *at_least_old != at_least_new {
					*self = Signature::Conflicting;
				}
			},
			(_, _) => {
				*self = Signature::Conflicting;
			},
		}
	}

	/// True if self is less specific than other and could be refined to match it
	pub(crate) fn could_be(&self, other: &Signature) -> bool {
		let mut refined = self.clone();
		refined.refine_with(other.clone());
		!matches!(refined, Signature::Conflicting)
	}
}

impl Signatures {
	pub(crate) fn new_empty() -> Self {
		Signatures(HashMap::new())
	}

	pub(crate) fn add_all_undefined_symbols_of_formula(&mut self, element: &ElementParsed) {
		match element {
			ElementParsed::Plus(elements)
			| ElementParsed::Multiply(elements)
			| ElementParsed::FunctionWithExpression { arguments: elements, .. } => {
				elements.iter().for_each(|e| self.add_all_undefined_symbols_of_formula(e))
			},
			ElementParsed::Pow(base, exponent) => {
				self.add_all_undefined_symbols_of_formula(base);
				self.add_all_undefined_symbols_of_formula(exponent);
			},
			ElementParsed::Negate(element) => {
				self.add_all_undefined_symbols_of_formula(element);
			},
			ElementParsed::Function { name, arguments } => {
				arguments.iter().for_each(|a| self.add_all_undefined_symbols_of_formula(a));

				let arg_signatures = arguments
					.iter()
					.map(|arg| match arg {
						ElementParsed::Number(_)
						| ElementParsed::Plus(_)
						| ElementParsed::Multiply(_)
						| ElementParsed::Pow(..)
						| ElementParsed::Negate(_)
						| ElementParsed::Variable(_) => Signature::Number,
						ElementParsed::VariableOrFunction(_) => Signature::NumberOrFunction,
						_ => panic!("Invalid element in function arguments: {:?}", arg),
					})
					.collect::<Vec<_>>();

				self.insert_or_replace_symbol(name, Signature::Function(arg_signatures));
			},
			ElementParsed::Variable(name) => self.insert_or_replace_symbol(name, Signature::Number),
			ElementParsed::VariableOrFunction(name) => {
				self.insert_or_replace_symbol(name, Signature::NumberOrFunction)
			},
			ElementParsed::Number(_) | ElementParsed::NumberWithExpression { .. } => {},
		}
	}

	fn insert_or_replace_symbol(&mut self, name: &String, val: Signature) {
		if let Some(entry) = self.get_mut(name) {
			entry.refine_with(val)
		} else {
			self.insert(name.clone(), val);
		}
	}

	pub(crate) fn refine_signature_and_undefined(
		opt_fun_args: &mut OptionalFunctionDeclarationArguments, undefined_signatures: &mut Signatures,
		formula: &ElementParsed, already_defined: &Signatures,
	) -> Result<()> {
		let parameter_names =
			opt_fun_args.as_ref().map(|v| v.names.iter().cloned().collect()).unwrap_or(HashSet::new());

		let all_undefined_names = undefined_signatures.keys().cloned().collect::<HashSet<_>>();
		for name in all_undefined_names {
			if parameter_names.contains(&name) {
				continue;
			}
			if let Some(already_defined_sig) = already_defined.get(&name) {
				if !undefined_signatures[&name].could_be(already_defined_sig)
					&& !already_defined_sig.could_be(&undefined_signatures[&name])
				{
					return Err(anyhow!(
						"The signature of {} is not compatible with the already defined signature: undefined: {:?} vs defined: {:?}",
						name,
						undefined_signatures[&name],
						already_defined_sig
					));
				}
				undefined_signatures.update_signature(formula, &name, already_defined_sig.clone())
			}
		}
		if let Some(args) = &mut opt_fun_args.0 {
			for (param_name, param_sig) in &mut args.signatures.0 {
				if let Some(var_sig_in_body) = undefined_signatures.get(param_name) {
					param_sig.refine_with(var_sig_in_body.clone())
				}
			}
		}
		undefined_signatures.retain(|n, _| !parameter_names.contains(n));
		undefined_signatures.retain(|n, _| !already_defined.contains_key(n));
		Ok(())
	}

	fn update_signature(
		&mut self, formula: &ElementParsed, element_to_update: &str, new_signature: Signature,
	) {
		let Some(signature) = self.get_mut(element_to_update) else { return };
		signature.refine_with(new_signature.clone());

		// update all functions that contain this symbol as a parameter
		let mut list_all_parameter_occurrences = HashSet::new();
		formula.list_all_functions_with_argument_variable(
			element_to_update,
			&mut list_all_parameter_occurrences,
		);
		for (fn_name, param_index) in list_all_parameter_occurrences {
			let mut new_fn_signature = self.0[&fn_name].clone();
			match &mut new_fn_signature {
				Signature::Function(args) => args[param_index] = new_signature.clone(),
				_ => panic!("This shouldn't happen"),
			}
			self.update_signature(formula, &fn_name, new_fn_signature);
		}

		if let Signature::Function(args) = new_signature {
			// update all parameters, of which their types may be affected
			let mut all_params_of_function_type = HashSet::new();
			formula.list_all_function_arguments_where_function_has_name(
				element_to_update,
				&mut all_params_of_function_type,
			);
			for (param_name, param_index) in all_params_of_function_type {
				let new_arg_signature = args[param_index].clone();
				self.get_mut(&param_name).unwrap().refine_with(new_arg_signature);
			}
		}
	}
}
impl ElementParsed {
	pub(crate) fn get_name(&self) -> Option<&str> {
		match self {
			ElementParsed::Function { name, .. }
			| ElementParsed::Variable(name)
			| ElementParsed::VariableOrFunction(name) => Some(name),
			_ => None,
		}
	}

	fn name_matches(&self, name: &str) -> bool {
		match self {
			ElementParsed::Function { name: name_cmp, .. }
			| ElementParsed::Variable(name_cmp)
			| ElementParsed::VariableOrFunction(name_cmp) => name_cmp == name,
			_ => false,
		}
	}

	/// Returns a set of Function names and indices, which parameter is equal to the ```name```.
	/// This is used to find all functions that take an Element with the given name as an argument.
	fn list_all_functions_with_argument_variable(&self, name: &str, list: &mut HashSet<(String, usize)>) {
		match self {
			ElementParsed::Function { arguments, name: this_name } => {
				for (i, _) in arguments.iter().enumerate().filter(|(_, e)| e.name_matches(name)) {
					list.insert((this_name.clone(), i));
				}
				arguments.iter().for_each(|a| a.list_all_functions_with_argument_variable(name, list))
			},
			ElementParsed::Plus(elements) | ElementParsed::Multiply(elements) => {
				elements.iter().for_each(|a| a.list_all_functions_with_argument_variable(name, list))
			},
			ElementParsed::Pow(a, b) => {
				a.list_all_functions_with_argument_variable(name, list);
				b.list_all_functions_with_argument_variable(name, list);
			},
			ElementParsed::Negate(e) => e.list_all_functions_with_argument_variable(name, list),
			ElementParsed::Number(_)
			| ElementParsed::NumberWithExpression { .. }
			| ElementParsed::FunctionWithExpression { .. }
			| ElementParsed::Variable(_)
			| ElementParsed::VariableOrFunction(_) => {},
		}
	}

	/// Returns a set of Function names and indices, which parameter is equal to the ```name```
	fn list_all_function_arguments_where_function_has_name(
		&self, name: &str, list: &mut HashSet<(String, usize)>,
	) {
		match self {
			ElementParsed::Function { arguments, name: this_name } => {
				if this_name == name {
					for (i, arg_name) in arguments
						.iter()
						.enumerate()
						.filter(|e| matches!(e.1, ElementParsed::VariableOrFunction(_) | ElementParsed::Variable(_)))
						.filter_map(|(i, e)| e.get_name().map(|n| (i, n)))
					{
						if arg_name != name {
							list.insert((arg_name.to_string(), i));
						}
					}
				}
				arguments
					.iter()
					.for_each(|a| a.list_all_function_arguments_where_function_has_name(name, list))
			}
			ElementParsed::Plus(elements) | ElementParsed::Multiply(elements) => elements
				.iter()
				.for_each(|e| e.list_all_function_arguments_where_function_has_name(name, list)),
			ElementParsed::Pow(a, b) => {
				a.list_all_function_arguments_where_function_has_name(name, list);
				b.list_all_function_arguments_where_function_has_name(name, list);
			}
			ElementParsed::Negate(e) => e.list_all_function_arguments_where_function_has_name(name, list),
			ElementParsed::Number(_)
			| ElementParsed::Variable(_)
			| ElementParsed::NumberWithExpression { .. }
			| ElementParsed::FunctionWithExpression { .. } // todo I'm not sure if it is correct to ignore this
			| ElementParsed::VariableOrFunction(_) => {}
		}
	}
}

impl FunctionDeclarationArguments {
	pub(crate) fn get_signatures_in_right_order(&self) -> Vec<Signature> {
		self.names.iter().map(|name| self.signatures.get(name).unwrap().clone()).collect::<Vec<_>>()
	}
}

impl SymbolDeclarationData {
	pub(crate) fn from_formula(formula: &ElementParsed) -> Result<Self> {
		let insert_name;
		let function_args;
		match formula {
			ElementParsed::Variable(name) | ElementParsed::VariableOrFunction(name) => {
				function_args = None;
				insert_name = name;
			},
			ElementParsed::Function { name, arguments } => {
				insert_name = name;
				let mut fn_args =
					FunctionDeclarationArguments { names: Vec::new(), signatures: Signatures::new_empty() };
				for arg in arguments {
					if let ElementParsed::VariableOrFunction(name) = arg {
						fn_args.names.push(name.clone());
						fn_args.signatures.insert(name.clone(), Signature::NumberOrFunction);
					} else {
						return Err(anyhow!("Invalid argument in function signature"));
					}
				}
				function_args = Some(fn_args)
			},
			_ => {
				return Err(anyhow!("Invalid formula signature provided"));
			},
		}
		Ok(Self {
			name: insert_name.to_owned(),
			function_args: OptionalFunctionDeclarationArguments(function_args),
		})
	}

	pub fn get_name(&self) -> &String {
		&self.name
	}
}

impl Deref for Signatures {
	type Target = HashMap<String, Signature>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl DerefMut for Signatures {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl Deref for OptionalFunctionDeclarationArguments {
	type Target = Option<FunctionDeclarationArguments>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl From<&SymbolDeclarationData> for Signature {
	fn from(value: &SymbolDeclarationData) -> Self {
		if let Some(args) = &value.function_args.0 {
			Signature::Function(args.get_signatures_in_right_order())
		} else {
			Signature::Number
		}
	}
}

impl From<HashMap<String, Signature>> for Signatures {
	fn from(value: HashMap<String, Signature>) -> Self {
		Signatures(value)
	}
}

impl FromIterator<(String, Signature)> for Signatures {
	fn from_iter<T: IntoIterator<Item = (String, Signature)>>(iter: T) -> Self {
		iter.into_iter().collect::<HashMap<_, _>>().into()
	}
}
