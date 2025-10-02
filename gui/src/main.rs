#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use crate::logic::Backend;
use crate::ui::UiState;
use crate::window_control::WindowState;
use egui::ViewportBuilder;

mod controller;
mod logic;
mod ui;
mod window_control;

pub fn main() {
    use eframe::NativeOptions;
    let viewport = ViewportBuilder::default()
        .with_decorations(false)
        .with_transparent(true)
        .with_taskbar(cfg!(debug_assertions))
        .with_always_on_top();

    eframe::run_native(
        "Quick Mafs",
        NativeOptions { viewport, ..Default::default() },
        Box::new(|cc| Ok(Box::new(Window::new(cc)))),
    )
    .expect("panic message");
}

pub struct Window {
    backend: Backend,
    ui_state: UiState,
    window_state: WindowState,
}
