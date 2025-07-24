
pub fn main() {
    use crate::gui_application::controller::Window;
    use eframe::NativeOptions;
    eframe::run_native("Window", NativeOptions::default(), Box::new(|cc| Ok(Box::new(Window::new(cc)))))
        .expect("panic message");
}

mod ui {
    pub(super) struct UiState {
        pub(super) top_user_input: String,
        pub(super) calculation_result: String,
    }

    impl UiState {
        pub(super) fn new() -> Self {
            Self { top_user_input: "".to_string(), calculation_result: "".to_string() }
        }
    }
}

mod logic {}

mod controller {
    use crate::gui_application::ui::UiState;
    use crate::libraries::storing::FormulaStore;
    use eframe::epaint::FontId;
    use eframe::{App, CreationContext, Frame};
    use egui::{CentralPanel, Context, FontFamily, FontSelection, RichText, ScrollArea, TextEdit};

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
                self.ui_state.calculation_result.clear();
                return;
            }
            if input.contains("=") {
                let result = self.formula_store.add_symbol_from_string(input, true);
                self.ui_state.calculation_result = match result {
                    Ok(name) => format!("Create new symbol '{}'", name),
                    Err(s) => format!("Error: {}", s),
                }
            } else {
                let result1 = self.formula_store.safe_eval(input);
                self.ui_state.calculation_result = match result1 {
                    Ok(result) => {
                        if result.is_lossy() {
                            format!("~= {}", result.value())
                        } else {
                            format!("= {}", result.value())
                        }
                    },
                    Err(s) => {
                        format!("Error: {}", s)
                    },
                }
            }
        }

        fn try_apply_calculation(&mut self) {
            let input = self.ui_state.top_user_input.trim();
            if input.is_empty() {
                self.ui_state.calculation_result.clear();
                return;
            }
            if input.contains("=") {
                let _ = self.formula_store.add_symbol_from_string(input, false);
                self.update_calculation_result();
            }
        }
    }

    impl App for Window {
        fn update(&mut self, ctx: &Context, frame: &mut Frame) {
            CentralPanel::default().show(ctx, |ui| {
                let edit = TextEdit::singleline(&mut self.ui_state.top_user_input)
                    .font(FontSelection::FontId(FontId::new(20.0, FontFamily::Proportional)));
                let response = ui.add_sized([ui.available_width(), 20.0], edit);
                if response.changed() {
                    self.update_calculation_result();
                }

                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.try_apply_calculation();
                    response.request_focus();
                }
                ui.label(RichText::new(&self.ui_state.calculation_result).size(14.0));
                let area = ScrollArea::vertical().auto_shrink(false);
                area.show(ui, |ui| {
                    self.formula_store.get_symbols().iter().for_each(|(name, params, value)| {
                        let mut text = name.to_string();
                        if let Some(params) = params {
                            text.push_str(&format!("({})", params.join(", ")));
                        }
                        text.push_str(" = ");
                        text.push_str(&value.to_string());
                        ui.label(RichText::new(text).size(14.0));
                    });
                });
            });
        }
    }
}
