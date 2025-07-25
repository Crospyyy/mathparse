pub fn main() {
    use crate::gui_application::controller::Window;
    use eframe::NativeOptions;
    eframe::run_native("Window", NativeOptions::default(), Box::new(|cc| Ok(Box::new(Window::new(cc)))))
        .expect("panic message");
}

mod ui {
    pub(super) struct UiState {
        pub(super) top_user_input: String,
        pub(super) calculation_result: Result<String, String>,
    }

    impl UiState {
        pub(super) fn new() -> Self {
            Self { top_user_input: "".to_string(), calculation_result: Ok("".to_string()) }
        }
    }
}

mod logic {}

mod controller {
    use crate::gui_application::ui::UiState;
    use crate::libraries::parsing::implementation::get_fun_name_end_of_string;
    use crate::libraries::storing::FormulaStore;
    use eframe::epaint::FontId;
    use eframe::{App, CreationContext, Frame};
    use egui::text::{CCursor, CCursorRange};
    use egui::{
        CentralPanel, Color32, Context, FontFamily, FontSelection, Label, Response, RichText, ScrollArea,
        TextEdit,
    };

    pub struct Window {
        formula_store: FormulaStore,
        ui_state: UiState,
    }

    impl Window {
        pub(crate) fn new(_cc: &CreationContext) -> Self {
            let mut store = FormulaStore::new_empty();
            store.define_default_internal_functions().unwrap();
            store.define_default_symbols().unwrap();
            Self { formula_store: store, ui_state: UiState::new() }
        }

        fn update_calculation_result(&mut self) {
            let input = self.ui_state.top_user_input.trim();
            if input.is_empty() {
                self.ui_state.calculation_result = Ok("".to_string());
                return;
            }
            if input.contains("=") {
                self.ui_state.calculation_result = self
                    .formula_store
                    .add_symbol_from_string(input, true)
                    .map(|name| format!("Create new symbol '{}'", name))
                    .map_err(|s| format!("Error: {}", s));
            } else {
                self.ui_state.calculation_result = self
                    .formula_store
                    .safe_eval(input)
                    .map(|result| {
                        if result.is_lossy() {
                            format!("~= {}", result.value())
                        } else {
                            format!("= {}", result.value())
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
                }
            }
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
                                if ui.input(|i| i.key_pressed(egui::Key::Tab))
                                    && var_name != longest_common_start
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
                ui.label(match &self.ui_state.calculation_result {
                    Ok(result) => RichText::new(result).size(20.0),
                    Err(err) => RichText::new(err).size(20.0).color(Color32::ORANGE.gamma_multiply(0.7)),
                });
                ui.separator();
                let area = ScrollArea::vertical().auto_shrink(false);
                area.show(ui, |ui| {
                    self.formula_store.get_symbols().iter().for_each(|(name, params, value)| {
                        let mut text = name.to_string();
                        if let Some(params) = params {
                            text.push_str(&format!("({})", params.join(", ")));
                        }
                        text.push_str(" = ");
                        text.push_str(&value.to_string());
                        ui.label(RichText::new(text).size(17.0));
                    });
                });
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
