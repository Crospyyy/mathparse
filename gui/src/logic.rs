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

    pub(crate) fn evaluate_formula(
        &mut self, ctx: &mut NumberContext, input: &String,
    ) -> Result<String, String> {
        let mut benchmark = Benchmark::new();
        self.formula_store
            .eval_with_benchmark(&input, ctx, &mut benchmark)
            .map(|result| {
                benchmark.print_times();
                let mut b = Benchmark::new();
                let result_str = benchmark!(
                    b,
                    result.to_string_detailed(
                        FormattingOptions::default().with_rounding(self.ui_state.output_digits),
                        ctx,
                    ),
                    "Formatting number"
                );
                b.print_times();

                match result_str {
                    NumberString::Imprecise(str) => format!("≈ {}", str),
                    NumberString::Precise { string, is_rounded } => {
                        if is_rounded {
                            format!("= {} (rounded)", string)
                        } else {
                            format!("= {}", string)
                        }
                    },
                }
            })
            .map_err(|s| format!("Error: {}", s))
    }
}

pub enum UiStateInfo {
    TopInputChanged,
    TopInputSubmit,
    RoundingAccuracyChanged,
    RequestAutocompletion { cursor_pos: usize, input_term: String, complete_to: String, response: Response },
    SelectPage(Page),
}
