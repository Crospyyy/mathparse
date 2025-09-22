#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use crate::ui::UiState;
use egui::{ViewportBuilder, WindowLevel};
use library::FormulaStore;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub fn main() {
    use eframe::NativeOptions;
    let viewport = ViewportBuilder::default()
        .with_decorations(false)
        .with_transparent(true)
        .with_taskbar(false)
        .with_always_on_top();

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
    pinned: bool,
}

impl WindowState {
    fn new() -> Self {
        Self {
            request_focus: Arc::new(AtomicBool::new(false)),
            last_frame_had_focus: false,
            centered: false,
            pinned: false,
        }
    }
}

mod controller;
mod logic;
mod ui;
