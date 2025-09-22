#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use crate::ui::UiState;
use egui::{Vec2, ViewportBuilder, WindowLevel};
use library::FormulaStore;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

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
    pinned: Arc<AtomicBool>,
    last_window_size: Arc<Mutex<Option<Vec2>>>,
}

impl WindowState {
    fn new() -> Self {
        Self {
            request_focus: Arc::new(AtomicBool::new(false)),
            last_frame_had_focus: false,
            centered: false,
            pinned: Arc::new(AtomicBool::new(false)),
            last_window_size: Arc::new(Mutex::new(None)),
        }
    }
    
    fn is_pinned(&self) -> bool {
        self.pinned.load(std::sync::atomic::Ordering::Relaxed)
    }
    
    fn set_pinned(&self, pinned: bool) {
        self.pinned.store(pinned, std::sync::atomic::Ordering::Relaxed);
    }
}

mod controller;
mod logic;
mod ui;
