use crate::Window;
use crate::ui::Page;
use egui::Response;
use library::{Benchmark, FormattingOptions, NamedSymbol, NumberContext, NumberString, benchmark};

impl Window {
    pub(crate) fn get_result_of_possible_symbol_declaration(
        &mut self, input: &str,
    ) -> Result<NamedSymbol, String> {
        self.formula_store.add_symbol_from_string(input, true)
    }
}

pub enum UiStateInfo {
    TopInputChanged,
    TopInputSubmit,
    RoundingAccuracyChanged,
    RequestAutocompletion { cursor_pos: usize, input_term: String, complete_to: String, response: Response },
    SelectPage(Page),
}
