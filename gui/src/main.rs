#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use crate::logic::Backend;
use crate::ui::UiState;
use crate::window_control::WindowState;
#[cfg(target_os = "linux")]
use eframe::EventLoopBuilderHook;
use egui::ViewportBuilder;
#[cfg(target_os = "linux")]
use x11rb::connect;

mod controller;
mod debugging;
mod logic;
mod ui;
mod window_control;
#[cfg(target_os = "linux")]
mod x11_activation;

pub fn main() {
	use eframe::NativeOptions;
	let viewport = ViewportBuilder::default()
		.with_decorations(false)
		.with_transparent(true)
		.with_taskbar(cfg!(debug_assertions));

	eframe::run_native(
		"Quick Mafs",
		NativeOptions {
			viewport,
			#[cfg(target_os = "linux")]
			event_loop_builder: x11_event_loop_builder(),
			..Default::default()
		},
		Box::new(|cc| Ok(Box::new(Window::new(cc)))),
	)
	.expect("panic message");
}

/// winit picks Wayland whenever a non-empty `WAYLAND_DISPLAY` or `WAYLAND_SOCKET` is set,
/// but skip-taskbar, keep-above, centering and `_NET_ACTIVE_WINDOW` only work on X11.
/// `None` when no X server (XWayland) is reachable; the app then runs on Wayland,
/// where the shortcut cannot unminimize or focus the window.
#[cfg(target_os = "linux")]
fn x11_event_loop_builder() -> Option<EventLoopBuilderHook> {
	if connect(None).is_err() {
		return None;
	}
	Some(Box::new(|builder| {
		use winit::platform::x11::EventLoopBuilderExtX11 as _;
		builder.with_x11();
	}))
}

pub struct Window {
	backend: Backend,
	ui_state: UiState,
	window_state: WindowState,
}
