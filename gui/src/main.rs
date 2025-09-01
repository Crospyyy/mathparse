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

mod controller;
mod logic;
mod ui;
