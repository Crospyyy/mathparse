#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use crate::controller::switch_visibility;
use crate::logic::Backend;
use crate::ui::UiState;
use egui::{Context, Vec2, ViewportBuilder, ViewportCommand, WindowLevel};
use global_shortcuts::register_global_shortcut;
use library::FormulaStore;
use std::collections::HashSet;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

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

    fn start_shortcut_listener(&self, ctx: &Context) {
        let ctx = ctx.clone();
        let req_focus = self.request_focus.clone();
        let last_window_size = self.last_window_size.clone();
        let pinned = self.pinned.clone();
        register_global_shortcut(global_shortcuts::Modifiers::ALT, global_shortcuts::Key::Space, move || {
            let mut window_size = last_window_size.clone().lock().unwrap().as_ref().copied();
            if pinned.load(std::sync::atomic::Ordering::Relaxed) {
                window_size = None;
            }
            switch_visibility(&ctx, true, window_size);
            ctx.send_viewport_cmd(ViewportCommand::Focus);
            req_focus.store(true, std::sync::atomic::Ordering::Relaxed);
        });
    }
}

mod controller;
mod logic;
mod ui;
