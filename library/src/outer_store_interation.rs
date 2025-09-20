use crate::evaluation::DynamicResult;
use crate::{FormulaStore, NamedSymbol};

impl FormulaStore {
    pub fn run(&mut self, input: impl ToString, dry_run: bool) -> RunResult {
        let input = input.to_string();
        if let Some((symbol, definition)) = input.split_once("=") {
            if symbol.contains('=') || definition.contains('=') {
                return RunResult::ParseFailed("Only one '=' is allowed in a formula".to_owned());
            }
            match self.add_symbol_from_string(&input, dry_run) {
                Ok(symbol) => RunResult::AddedSymbol(symbol),
                Err(err) => RunResult::FailedToAddSymbol(err),
            }
        } else {
            match self.eval_dynamic_precision(&input, 512..=(1 << 20)) {
                Ok(result) => RunResult::CalculationResult(result),
                Err(err) => RunResult::CalculationFailed(err),
            }
        }
    }
}

pub enum RunResult {
    ParseFailed(String),
    CalculationFailed(String),
    FailedToAddSymbol(String),
    CalculationResult(DynamicResult),
    AddedSymbol(NamedSymbol),
}

impl RunResult {
    pub fn calculation_result(self) -> Option<DynamicResult> {
        match self {
            RunResult::CalculationResult(res) => Some(res),
            _ => None,
        }
    }
}
