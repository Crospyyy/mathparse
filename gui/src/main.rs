use crate::controller::Window;

pub fn main() {
    use eframe::NativeOptions;
    eframe::run_native("Quick Mafs", NativeOptions::default(), Box::new(|cc| Ok(Box::new(Window::new(cc)))))
        .expect("panic message");
}

mod ui {
    use library::Number;

    pub(super) struct UiState {
        pub(super) top_user_input: String,
        pub(super) calculation_result: Result<String, String>,
        pub(super) output_digits: usize,
        pub all_symbol_strings: Vec<String>,
    }

    impl UiState {
        pub(super) fn new() -> Self {
            let state = Self {
                top_user_input: "".to_string(),
                calculation_result: Ok("".to_string()),
                output_digits: Number::DEFAULT_ROUNDING_DIGITS,
                all_symbol_strings: Vec::new(),
            };
            state
        }
    }
}

mod logic {}

mod controller {
    use crate::ui::UiState;
    use eframe::epaint::FontId;
    use eframe::epaint::text::TextWrapMode;
    use eframe::{App, CreationContext, Frame};
    use egui::text::{CCursor, CCursorRange, LayoutJob, TextWrapping};
    use egui::{
        CentralPanel, Color32, Context, DragValue, FontFamily, FontSelection, Label, Response, RichText,
        ScrollArea, TextEdit, TextFormat, Ui, Widget,
    };
    use library::operations::create_default_context;
    use library::parsing::implementation::get_fun_name_end_of_string;
    use library::storing::FormulaStore;

    pub struct Window {
        formula_store: FormulaStore,
        ui_state: UiState,
    }

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

        fn update_calculation_result(&mut self) {
            let ctx = &mut create_default_context();
            let input = self.ui_state.top_user_input.trim();
            if input.is_empty() {
                self.ui_state.calculation_result = Ok("".to_string());
                return;
            }
            if input.contains("=") {
                self.ui_state.calculation_result = self
                    .formula_store
                    .add_symbol_from_string(input, true)
                    .map(|name| format!("Create new symbol '{}'", name));
            } else {
                self.ui_state.calculation_result = self
                    .formula_store
                    .eval(input, ctx)
                    .map(|result| {
                        let result_str = result.to_string(self.ui_state.output_digits, ctx);
                        if result.is_exact() {
                            format!("= {}", result_str)
                        } else {
                            format!("~= {}", result_str)
                        }
                    })
                    .map_err(|s| format!("Error: {}", s));
            }
        }

        fn try_apply_calculation(&mut self) {
            let input = self.ui_state.top_user_input.trim();
            if input.is_empty() {
                return;
            }
            if input.contains("=") {
                let result = self.formula_store.add_symbol_from_string(input, false);
                if result.is_ok() {
                    self.ui_state.top_user_input.clear();
                    self.update_calculation_result();
                    self.update_all_symbol_strings();
                }
            }
        }

        fn show_top_input(&mut self, ui: &mut Ui) {
            let edit = TextEdit::singleline(&mut self.ui_state.top_user_input)
                .font(FontSelection::FontId(FontId::new(20.0, FontFamily::Proportional)))
                .lock_focus(true);
            let response = ui.add_sized([ui.available_width(), 20.0], edit);
            if response.has_focus() {
                let var_name = get_fun_name_end_of_string(&self.ui_state.top_user_input);
                if !var_name.is_empty() {
                    let compatible_symbols = self
                        .formula_store
                        .get_symbols()
                        .iter()
                        .filter(|(name, ..)| name.starts_with(&var_name))
                        .map(|(name, ..)| *name)
                        .collect::<Vec<_>>();

                    if !compatible_symbols.is_empty() {
                        let longest_common_start = determine_longest_common_start(&compatible_symbols);
                        if !(compatible_symbols.len() == 1 && compatible_symbols[0] == &var_name) {
                            response.show_tooltip_ui(|ui| {
                                for name in compatible_symbols {
                                    ui.add(Label::new(name).extend());
                                }
                            });
                            if ui.input(|i| i.key_pressed(egui::Key::Tab)) && var_name != longest_common_start
                            {
                                self.ui_state.top_user_input += &longest_common_start[var_name.len()..];
                                self.update_calculation_result();
                                set_cursor_pos(&response, self.ui_state.top_user_input.len());
                            }
                        }
                    }
                }
            }
            if response.changed() {
                self.update_calculation_result();
            }
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.try_apply_calculation();
                response.request_focus();
            }

            let mut job = LayoutJob::default();
            let calc_result = &self.ui_state.calculation_result;
            let text = match calc_result {
                Ok(result) => result,
                Err(err) => err,
            };
            let mut format =
                TextFormat { font_id: FontId::new(20.0, FontFamily::Proportional), ..Default::default() };
            if calc_result.is_err() {
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
                    self.update_calculation_result();
                }
            });
        }

        fn show_all_defined_symbols(&mut self, ui: &mut Ui) {
            let area = ScrollArea::vertical().auto_shrink(false);
            area.show(ui, |ui| {
                for text in &self.ui_state.all_symbol_strings {
                    ui.label(RichText::new(text).size(17.0));
                }
            });
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
    }

    fn set_cursor_pos(response: &Response, cursor_pos: usize) {
        if let Some(mut state) = TextEdit::load_state(&response.ctx, response.id) {
            state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(cursor_pos))));
            state.store(&response.ctx, response.id);
        }
    }

    impl App for Window {
        fn update(&mut self, ctx: &Context, frame: &mut Frame) {
            CentralPanel::default().show(ctx, |ui| {
                self.show_top_input(ui);
                ui.separator();
                self.show_all_defined_symbols(ui);
            });
        }
    }

    fn determine_longest_common_start(names: &Vec<&String>) -> String {
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
