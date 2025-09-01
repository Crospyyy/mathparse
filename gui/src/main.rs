use crate::ui::UiState;
use egui::ViewportBuilder;
use library::FormulaStore;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub fn main() {
    use eframe::NativeOptions;
    let viewport =
        ViewportBuilder::default().with_always_on_top().with_decorations(false).with_transparent(true);
    eframe::run_native(
        "Quick Mafs",
        NativeOptions { viewport, ..Default::default() },
        Box::new(|cc| Ok(Box::new(Window::new(cc)))),
    )
    .expect("panic message");
}

pub struct Window {
    formula_store: FormulaStore,
    ui_state: UiState,
    window_state: WindowState,
}

struct WindowState {
    request_focus: Arc<AtomicBool>,
    last_frame_had_focus: bool,
    centered: bool,
}

impl WindowState {
    fn new() -> Self {
        Self { request_focus: Arc::new(AtomicBool::new(false)), last_frame_had_focus: false, centered: false }
    }
}

mod controller;
mod logic;
mod ui;
