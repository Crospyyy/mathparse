use crate::Window;
#[cfg(target_os = "linux")]
use crate::x11_activation::ActivateWindow;
use eframe::CreationContext;
use eframe::emath::Vec2;
use egui::{
	Context, Event, Id, Key, PointerButton, PointerState, RawInput, Ui, ViewportCommand, WindowLevel,
};
#[cfg(windows)]
use global_shortcuts::register_global_shortcut;
use library::{debug_print, only_in_debug};
#[cfg(target_os = "linux")]
use linux_shortcuts::non_async_global_shortcuts;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub fn switch_visibility(ctx: &Context, visible: bool, last_window_size: Option<Vec2>) {
	if visible {
		ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
		if last_window_size.is_some() {
			let size = Window::DEFAULT_WINDOW_SIZE;
			try_center_window(ctx, Some(size));
			ctx.send_viewport_cmd(ViewportCommand::InnerSize(size))
		}
		// ctx.send_viewport_cmd(ViewportCommand::OuterPosition([0.0; 2].into()));
		ctx.send_viewport_cmd(ViewportCommand::Focus);
	} else {
		// ctx.send_viewport_cmd(ViewportCommand::OuterPosition([0.0, 10000.0].into()));
		ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
	}
}

pub fn try_center_window(ctx: &Context, last_window_size: Option<Vec2>) -> bool {
	let (monitor_opt, win_size_opt) = ctx.input(|i| (i.viewport().monitor_size, last_window_size));

	only_in_debug! {
		dbg!(monitor_opt);
		dbg!(win_size_opt);
	}

	if let (Some(monitor), Some(win_size)) = (monitor_opt, win_size_opt) {
		let pos = (monitor - win_size) / 2.0;
		ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pos.to_pos2()));
		debug_print!("Center window at: {:?}", pos);
		return true;
	}
	false
}

impl Window {
	pub(crate) const DEFAULT_WINDOW_SIZE: Vec2 = Vec2::new(528.3, 386.7);

	pub(crate) fn raw_input_hook_inner(&mut self, ctx: &Context, raw_input: &mut RawInput) {
		if !self.window_state.is_pinned() {
			let is_focussed = ctx.memory(|m| m.focused().is_none());
			let pressed_escape = raw_input
				.events
				.iter()
				.any(|e| matches!(e, Event::Key { key: Key::Escape, pressed: true, repeat: false, .. }));
			if is_focussed && pressed_escape {
				switch_visibility(ctx, false, self.get_last_window_size());
			}
		}

		// match alt + space
		if ctx.input(|ip|ip.modifiers.alt) // todo find a way to prevent the windows window menu from opening
			&& raw_input
			.events
			.iter()
			.any(|x| matches!(x, Event::Key { key: Key::Space, pressed: true, repeat: false, .. }))
		{
			raw_input.events.retain(|e| {
				!matches!(e, Event::Key { key: Key::Space, .. }) && !matches!(e, Event::Text(t) if t == " ")
			});
			self.request_top_input_focus(ctx);
		}
	}

	pub(crate) fn get_last_window_size(&self) -> Option<Vec2> {
		*self.window_state.last_window_size.lock().unwrap()
	}

	pub(super) fn handle_window_control(&mut self, ctx: &Context, pressed_shortcut: &mut bool) {
		// store last window size
		let mut guard = self.window_state.last_window_size.lock().unwrap();
		let option = ctx.input(|i| i.viewport().outer_rect.map(|r| r.size()));
		if let Some(size) = option {
			*guard = Some(size);
		};
		drop(guard);

		// handle focus request
		if self.window_state.request_focus.load(Ordering::Relaxed) {
			self.window_state.request_focus.store(false, Ordering::Relaxed);
			self.request_top_input_focus(ctx);
			*pressed_shortcut = true;
		}
		let has_focus = ctx.input(|ip| ip.raw.focused);
		if !self.window_state.is_pinned() && self.window_state.last_frame_had_focus && !has_focus {
			switch_visibility(ctx, false, self.get_last_window_size());
		}
		self.window_state.last_frame_had_focus = has_focus;
	}

	pub fn switch_window_pinning(&mut self, ctx: &Context) {
		let new_pinned = !self.window_state.is_pinned();
		ctx.send_viewport_cmd(ViewportCommand::WindowLevel(if new_pinned {
			WindowLevel::AlwaysOnTop
		} else {
			WindowLevel::Normal
		}));
		ctx.send_viewport_cmd(ViewportCommand::Decorations(new_pinned));
		self.window_state.set_pinned(new_pinned);
		ctx.send_viewport_cmd(ViewportCommand::Transparent(!new_pinned));
		if !new_pinned {
			switch_visibility(ctx, false, self.get_last_window_size());
		} else {
			ctx.send_viewport_cmd(ViewportCommand::InnerSize(Window::DEFAULT_WINDOW_SIZE));
		}
	}

	pub fn window_background_logic(&mut self, ui: &mut Ui) {
		let resp = ui.interact(ui.max_rect(), Id::unique("window-drag-bg"), egui::Sense::click_and_drag());
		if resp.drag_started_by(PointerButton::Primary) {
			ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
			ui.input_mut(|i| i.pointer = PointerState::default());
			ui.ctx().stop_dragging();
		} else {
			resp.context_menu(|ui| {
				self.show_background_context_menu(ui);
			});
		}
	}
}

pub struct WindowState {
	request_focus: Arc<AtomicBool>,
	last_frame_had_focus: bool,
	pinned: Arc<AtomicBool>,
	pub(crate) last_window_size: Arc<Mutex<Option<Vec2>>>,
}

impl WindowState {
	pub(crate) fn new() -> Self {
		Self {
			request_focus: Arc::new(AtomicBool::new(false)),
			last_frame_had_focus: false,
			pinned: Arc::new(AtomicBool::new(false)),
			last_window_size: Arc::new(Mutex::new(None)),
		}
	}

	pub(crate) fn is_pinned(&self) -> bool {
		self.pinned.load(Ordering::Relaxed)
	}

	fn set_pinned(&self, pinned: bool) {
		self.pinned.store(pinned, Ordering::Relaxed);
	}

	pub(crate) fn start_shortcut_listener(&self, cc: &CreationContext<'_>) {
		let ctx = cc.egui_ctx.clone();
		#[cfg(target_os = "linux")]
		let activation = ActivateWindow::from_cc(cc);
		#[cfg(target_os = "linux")]
		if let Some(activation) = activation
			&& !cfg!(debug_assertions)
			&& let Err(err) = activation.skip_taskbar()
		{
			eprintln!("[window] skipping taskbar failed: {err:#}");
		}
		let req_focus = self.request_focus.clone();
		let last_window_size = self.last_window_size.clone();
		let pinned = self.pinned.clone();
		let callback = move || {
			let mut window_size = last_window_size.clone().lock().unwrap().as_ref().copied();
			if pinned.load(Ordering::Relaxed) {
				window_size = None;
			}
			switch_visibility(&ctx, true, window_size);
			// Sent now, while the commands above are only queued: the WM
			// unminimizes and focuses us before they are applied.
			#[cfg(target_os = "linux")]
			if let Some(activation) = activation
				&& let Err(err) = activation.activate()
			{
				eprintln!("[window_control] activation failed: {err:#}");
			}
			req_focus.store(true, Ordering::Relaxed);
		};
		#[cfg(windows)]
		register_global_shortcut(global_shortcuts::Modifiers::ALT, global_shortcuts::Key::Space, callback);
		#[cfg(target_os = "linux")]
		non_async_global_shortcuts(callback);
	}
}
