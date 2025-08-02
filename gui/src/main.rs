use crate::ui::UiState;
use library::FormulaStore;

pub fn main() {
    use eframe::NativeOptions;
    eframe::run_native("Quick Mafs", NativeOptions::default(), Box::new(|cc| Ok(Box::new(Window::new(cc)))))
        .expect("panic message");
}

pub struct Window {
    formula_store: FormulaStore,
    ui_state: UiState,
}

mod ui {
    use crate::Window;
    use crate::controller::get_cursor_pos;
    use crate::logic::UiStateInfo;
    use eframe::epaint::text::{LayoutJob, TextFormat, TextWrapping};
    use eframe::epaint::{Color32, FontFamily, FontId};
    use eframe::{App, Frame};
    use egui::{
        CentralPanel, Context, DragValue, FontSelection, Label, Response, RichText, ScrollArea, TextEdit, Ui,
        Widget,
    };
    use library::FormattingOptions;

    pub(super) struct UiState {
        pub(super) top_user_input: String,
        pub(super) calculation_result: Option<Result<String, String>>,
        pub(super) output_digits: usize,
        pub all_symbol_strings: Vec<String>,
    }

    impl UiState {
        pub(super) fn new() -> Self {
            Self {
                top_user_input: "".to_string(),
                calculation_result: None,
                output_digits: FormattingOptions::default().round_to_decimals,
                all_symbol_strings: Vec::new(),
            }
        }
    }

    impl App for Window {
        fn update(&mut self, ctx: &Context, frame: &mut Frame) {
            CentralPanel::default().show(ctx, |ui| {
                let input = &mut vec![];
                self.show_top_input(ui, input);
                ui.separator();
                self.show_all_defined_symbols(ui);
                self.handle_ui_input(input);
            });
        }
    }

    impl Window {
        fn show_autocompletion(&mut self, ui: &mut Ui, response: &Response, input: &mut Vec<UiStateInfo>) {
            let Some(cursor_pos) = get_cursor_pos(&response) else { return };
            let Some(autocompletion) =
                self.get_autocompletion_result(&self.ui_state.top_user_input, cursor_pos)
            else {
                return;
            };

            response.show_tooltip_ui(|ui| {
                for name in autocompletion.possible_symbols {
                    ui.add(Label::new(name).extend());
                }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Tab)) {
                input.push(UiStateInfo::RequestAutocompletion {
                    cursor_pos,
                    input_term: autocompletion.input_term,
                    complete_to: autocompletion.longest_common_start,
                    response: response.clone(),
                });
            }
        }

        fn show_all_defined_symbols(&mut self, ui: &mut Ui) {
            let area = ScrollArea::vertical().auto_shrink(false);
            area.show(ui, |ui| {
                for text in &self.ui_state.all_symbol_strings {
                    ui.label(RichText::new(text).size(17.0));
                }
            });
        }

        pub(crate) fn show_top_input(&mut self, ui: &mut Ui, input: &mut Vec<UiStateInfo>) {
            let text_edit = TextEdit::singleline(&mut self.ui_state.top_user_input)
                .font(FontSelection::FontId(FontId::new(20.0, FontFamily::Proportional)))
                .lock_focus(true);
            let response = ui.add_sized([ui.available_width(), 20.0], text_edit);
            if response.has_focus() {
                self.show_autocompletion(ui, &response, input);
            }
            self.handle_ui_input(input);

            if response.changed() {
                input.push(UiStateInfo::TopInputChanged);
            }
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                response.request_focus();
                input.push(UiStateInfo::TopInputSubmit);
            }

            self.handle_ui_input(input);

            let mut job = LayoutJob::default();
            let text = match &self.ui_state.calculation_result {
                Some(Ok(result)) => result,
                Some(Err(err)) => err,
                None => "",
            };

            let mut format =
                TextFormat { font_id: FontId::new(20.0, FontFamily::Proportional), ..Default::default() };
            if self.ui_state.calculation_result.as_ref().is_some_and(|r| r.is_err()) {
                format.color = Color32::ORANGE.gamma_multiply(0.7);
            }
            job.append(text, 0.0, format);
            job.wrap = TextWrapping {
                max_width: ui.available_width(),
                max_rows: usize::MAX,
                break_anywhere: true,
                overflow_character: None,
            };

            ui.label(job);
            ui.horizontal(|ui| {
                ui.label("Round Digits:");
                let drag_val_resp = DragValue::new(&mut self.ui_state.output_digits).range(1..=100).ui(ui);
                if drag_val_resp.changed() {
                    input.push(UiStateInfo::RoundingAccuracyChanged);
                }
            });
        }
    }
}

mod logic {
    use crate::Window;
    use egui::Response;
    use library::{Benchmark, FormattingOptions, NumberContext, benchmark};

    impl Window {
        pub(crate) fn get_result_of_possible_symbol_declaration(
            &mut self, input: &str,
        ) -> Result<String, String> {
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
                    benchmark!(b, "Formatting number",
                        let result_str = result.to_string(
                            FormattingOptions::default().with_rounding(self.ui_state.output_digits),
                            ctx,
                        )
                    );
                    b.print_times();

                    if result.is_exact() { format!("= {}", result_str) } else { format!("~= {}", result_str) }
                })
                .map_err(|s| format!("Error: {}", s))
        }
    }

    pub enum UiStateInfo {
        TopInputChanged,
        TopInputSubmit,
        RoundingAccuracyChanged,
        RequestAutocompletion {
            cursor_pos: usize,
            input_term: String,
            complete_to: String,
            response: Response,
        },
    }
}

mod controller {
    use crate::Window;
    use crate::logic::UiStateInfo;
    use crate::ui::UiState;
    use eframe::CreationContext;
    use egui::text::{CCursor, CCursorRange};
    use egui::{Response, TextEdit, Widget};
    use library::{FormulaStore, create_default_context, get_fun_name_end_of_string};

    impl Window {
        pub(crate) fn new(_cc: &CreationContext) -> Self {
            let mut store = FormulaStore::new_empty();
            store.define_default_symbols().unwrap();
            store.add_variable_with_value("speed_of_sound_mps", "343", false).unwrap();
            store.add_variable_with_value("speed_of_light_mps", "299_792_458", false).unwrap();
            store.add_variable_with_value("kw_to_ps", "1.35962", false).unwrap();
            store.add_variable_with_value("km_to_miles", "0.6214", false).unwrap();
            store.add_variable_with_value("liter_to_gallons", "0.264172", false).unwrap();
            let mut window = Self { formula_store: store, ui_state: UiState::new() };
            window.update_all_symbol_strings();
            window
        }

        pub fn handle_ui_input(&mut self, input: &mut Vec<UiStateInfo>) {
            for info in input.drain(..) {
                match info {
                    UiStateInfo::TopInputSubmit => self.try_apply_calculation(),
                    UiStateInfo::TopInputChanged | UiStateInfo::RoundingAccuracyChanged => {
                        self.update_calculation_result()
                    },
                    UiStateInfo::RequestAutocompletion { cursor_pos, input_term, complete_to, response } => {
                        self.ui_state.top_user_input.insert_str(cursor_pos, &complete_to[input_term.len()..]);
                        self.update_calculation_result();
                        set_cursor_pos(&response, cursor_pos + complete_to.len() - input_term.len());
                    },
                }
            }
            self.update_all_symbol_strings();
        }

        fn update_calculation_result(&mut self) {
            let ctx = &mut create_default_context();
            let input = self.ui_state.top_user_input.trim().to_string();

            self.ui_state.calculation_result = if input.is_empty() {
                None
            } else if input.contains("=") {
                Some(
                    self.get_result_of_possible_symbol_declaration(&input)
                        .map(|name| format!("Create new symbol '{}'", name)),
                )
            } else {
                Some(self.evaluate_formula(ctx, &input))
            }
        }

        fn try_apply_calculation(&mut self) {
            let input = self.ui_state.top_user_input.trim();
            if input.is_empty() || !input.contains("=") {
                return;
            }
            if self.formula_store.add_symbol_from_string(input, false).is_ok() {
                self.ui_state.top_user_input.clear();
                self.update_calculation_result();
                self.update_all_symbol_strings();
            }
        }

        fn update_all_symbol_strings(&mut self) {
            let mut elements = self.formula_store.get_symbols();
            elements.sort_by_key(|e| e.0);
            self.ui_state.all_symbol_strings = elements
                .iter()
                .map(|(name, params, value)| {
                    let mut text = name.to_string();
                    if let Some(params) = params {
                        text.push_str(&format!("({})", params.join(", ")));
                    }
                    text.push_str(" = ");
                    text.push_str(&value.get_string(&mut create_default_context()));
                    text
                })
                .collect();
        }

        pub fn get_autocompletion_result(
            &self, input_string: &str, cursor_pos: usize,
        ) -> Option<AutocompletionResult> {
            let input_symbol_name = get_fun_name_end_of_string(&input_string[..cursor_pos]);
            if input_symbol_name.is_empty() {
                return None;
            }
            let compatible_symbols = self
                .formula_store
                .get_symbols()
                .iter()
                .filter(|(name, ..)| name.starts_with(&input_symbol_name))
                .map(|(name, ..)| *name)
                .collect::<Vec<_>>();

            if compatible_symbols.is_empty()
                || (compatible_symbols.len() == 1 && compatible_symbols[0] == &input_symbol_name)
            {
                return None;
            }
            let longest_common_start = determine_longest_common_start(&compatible_symbols);
            Some(AutocompletionResult {
                input_term: input_symbol_name,
                possible_symbols: compatible_symbols,
                longest_common_start,
            })
        }
    }

    pub struct AutocompletionResult<'a> {
        pub input_term: String,
        pub possible_symbols: Vec<&'a String>,
        pub longest_common_start: String,
    }

    fn set_cursor_pos(response: &Response, cursor_pos: usize) {
        if let Some(mut state) = TextEdit::load_state(&response.ctx, response.id) {
            state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(cursor_pos))));
            state.store(&response.ctx, response.id);
        }
    }

    pub fn get_cursor_pos(response: &Response) -> Option<usize> {
        if let Some(state) = TextEdit::load_state(&response.ctx, response.id) {
            state.cursor.char_range().and_then(|c| c.single()).map(|c| c.index)
        } else {
            None
        }
    }

    pub fn determine_longest_common_start(names: &Vec<&String>) -> String {
        if names.is_empty() {
            return String::new();
        }
        let common_start = names[0].clone();
        let mut longest_common = common_start.len();
        for name in names.iter().skip(1) {
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
}
