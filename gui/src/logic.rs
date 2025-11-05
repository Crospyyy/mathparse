use crate::ui::bottom_panel::Page;
use anyhow::Result;
use egui::Response;
use library::{Benchmark, FormulaStore, RunOptions, RunResult, RunSuccess, Symbol, only_in_debug};
use std::collections::HashSet;

pub struct Backend {
	custom_symbols: HashSet<String>,
	formula_store: FormulaStore,
}

impl Backend {
	pub fn new() -> Self {
		let mut store = FormulaStore::new_with_default_symbols();
		define_define_preset_symbols(&mut store).unwrap();
		Self { custom_symbols: HashSet::new(), formula_store: store }
	}

	pub fn get_symbol(&self, name: &str) -> Option<&Symbol> {
		self.formula_store.get_symbol(name)
	}

	pub fn has_custom_symbols(&self) -> bool {
		!self.custom_symbols.is_empty()
	}

	pub fn clear_custom_symbols(&mut self) {
		self.custom_symbols.clear();
		self.formula_store = FormulaStore::new_with_default_symbols(); // todo add logic to remove only custom symbols
	}

	pub(crate) fn formula_store(&self) -> &FormulaStore {
		&self.formula_store
	}

	pub fn dry_run(&mut self, input: &str) -> RunResult {
        let mut benchmark = Benchmark::new("Dry run");
        let result = self.formula_store.run_new(
			input,
			RunOptions { dry_run: true, ..Default::default() },
            &mut benchmark,
        );
        benchmark.finalize().print();
        result
	}

	pub fn run(&mut self, input: &str) -> RunResult {
		let mut benchmark = Benchmark::new("Running input");
		let result = self.formula_store.run_new(input, RunOptions::default(), &mut benchmark);
		only_in_debug!({
			let task = benchmark.finalize();
			task.print();
		});
		if let RunResult::Ok(RunSuccess::AddedSymbol(s)) = &result {
			self.custom_symbols.insert(s.name().clone());
		}
		result
	}
}

fn define_define_preset_symbols(store: &mut FormulaStore) -> Result<()> {
	let preset_symbols = [
		"speed_of_sound_mps = 343",
		"speed_of_light_mps = 299_792_458",
		"kw_to_ps = 1.35962",
		"km_to_miles = 0.6214",
		"liter_to_gallons = 0.264172",
		"joule_to_wh = 1/3600",
		"water_heat_capacity_j_per_g = 4.184",
	];
	for symbol in preset_symbols {
		store.add_symbol_from_string(symbol, false)?;
	}
	Ok(())
}

pub enum UiInteraction {
	TopInputChanged,
	TopInputSubmit,
	RoundingAccuracyChanged,
	RequestAutocompletion { cursor_pos: usize, input_term: String, complete_to: String, response: Response },
	SelectPage(Page),
	ClearCustomSymbols,
	ClearHistory,
}

pub fn determine_longest_common_start(names: &[(&String, &Symbol)]) -> String {
	if names.is_empty() {
		return String::new();
	}
	let common_start = names[0].0.to_string();
	let mut longest_common = common_start.len();
	for name in names.iter().map(|(n, _)| n).skip(1) {
		if longest_common == 0 {
			return String::new();
		}
		let max = longest_common.min(name.len());
		longest_common = max;
		for i in 0..max {
			if common_start.chars().nth(i) != name.chars().nth(i) {
				longest_common = i;
				break;
			}
		}
	}
	common_start[..longest_common].to_string()
}
