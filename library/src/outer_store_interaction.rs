use crate::benchmarking::Benchmark;
use crate::evaluation::DynamicResult;
use crate::{FormulaStore, NamedSymbol};

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
	pub fn run_new(
		&mut self, input: impl ToString, run_options: RunOptions, bench: &mut Benchmark,
	) -> RunResult {
		let input = input.to_string();
		if let Some((symbol, definition)) = input.split_once("=") {
			if symbol.contains('=') || definition.contains('=') {
				return RunError::ParseFailed("Only one '=' is allowed in a formula".to_owned()).into();
			}
			match bench.benchmark("Add symbol from string", || {
				self.add_symbol_from_string(&input, run_options.dry_run)
			}) {
				Ok(symbol) => RunSuccess::AddedSymbol(symbol).into(),
				Err(err) => RunError::FailedToAddSymbol(err.to_string()).into(),
			}
		} else {
			match bench.bench_with_inner("Evaluation", |b| self.eval_new(&input, run_options.precision, b)) {
				Ok(result) => RunSuccess::CalculationResult(result).into(),
				Err(err) => RunError::CalculationFailed(err.to_string()).into(),
			}
		}
	}
}

#[derive(Copy, Clone)]
pub enum RunPrecision {
	/// Minimum and maximum precision (in bits) to try.
	Dynamic(usize, usize),
	Fixed(usize),
}

impl Default for RunPrecision {
	fn default() -> Self {
		RunPrecision::Dynamic(512, 1 << 20)
	}
}

#[derive(Copy, Clone)]
pub struct RunOptions {
	pub dry_run: bool,
	pub precision: RunPrecision,
}

impl Default for RunOptions {
	/// Default options for running inputs.
	/// - `dry_run`: false
	/// - `precision`: default
	fn default() -> Self {
		RunOptions { dry_run: false, precision: RunPrecision::default() }
	}
}
