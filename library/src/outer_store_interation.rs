use crate::evaluation::DynamicResult;
use crate::{FormulaStore, NamedSymbol};
use std::ops::RangeInclusive;

impl FormulaStore {
	pub fn run(&mut self, input: impl ToString, dry_run: bool) -> RunResult {
		let input = input.to_string();
		if let Some((symbol, definition)) = input.split_once("=") {
			if symbol.contains('=') || definition.contains('=') {
				return RunError::ParseFailed("Only one '=' is allowed in a formula".to_owned()).into();
			}
			match self.add_symbol_from_string(&input, dry_run) {
				Ok(symbol) => RunSuccess::AddedSymbol(symbol).into(),
				Err(err) => RunError::FailedToAddSymbol(err.to_string()).into(),
			}
		} else {
			match self.eval_dynamic_precision(&input, FormulaStore::DEFAULT_PRECISION_RANGE) {
				Ok(result) => RunSuccess::CalculationResult(result).into(),
				Err(err) => RunError::CalculationFailed(err.to_string()).into(),
			}
		}
	}
}

pub enum RunResult {
	Ok(RunSuccess),
	Err(RunError),
}

pub enum RunSuccess {
	AddedSymbol(NamedSymbol),
	CalculationResult(DynamicResult),
}

pub enum RunError {
	ParseFailed(String),
	CalculationFailed(String),
	FailedToAddSymbol(String),
}

impl RunResult {
	pub fn calculation_result(self) -> Option<DynamicResult> {
		match self {
			RunResult::Ok(RunSuccess::CalculationResult(res)) => Some(res),
			_ => None,
		}
	}
}

impl RunError {
	fn to_string(self) -> String {
		match self {
			RunError::ParseFailed(s) | RunError::CalculationFailed(s) | RunError::FailedToAddSymbol(s) => s,
		}
	}
}

impl From<RunSuccess> for RunResult {
	fn from(value: RunSuccess) -> Self {
		RunResult::Ok(value)
	}
}

impl From<RunError> for RunResult {
	fn from(value: RunError) -> Self {
		RunResult::Err(value)
	}
}

// new public interface for formula store
impl FormulaStore {
	pub fn run_new(&mut self, input: impl ToString, run_options: RunOptions) -> RunResult {
		let input = input.to_string();
		if let Some((symbol, definition)) = input.split_once("=") {
			if symbol.contains('=') || definition.contains('=') {
				return RunError::ParseFailed("Only one '=' is allowed in a formula".to_owned()).into();
			}
			match self.add_symbol_from_string(&input, run_options.dry_run) {
				Ok(symbol) => RunSuccess::AddedSymbol(symbol).into(),
				Err(err) => RunError::FailedToAddSymbol(err.to_string()).into(),
			}
		} else {
			match self.eval_dynamic_precision(&input, FormulaStore::DEFAULT_PRECISION_RANGE) {
				Ok(result) => RunSuccess::CalculationResult(result).into(),
				Err(err) => RunError::CalculationFailed(err.to_string()).into(),
			}
		}
	}
}

pub enum RunPrecision {
	Dynamic(RangeInclusive<u32>),
	Fixed(u32),
}

pub struct RunOptions {
	pub dry_run: bool,
	pub precision_range: RunPrecision,
}
