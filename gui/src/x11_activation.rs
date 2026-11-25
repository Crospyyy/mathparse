//! Unminimize + focus our window through the window manager.
//!
//! `ViewportCommand::Focus` does nothing while the window is minimized (winit only
//! sends `_NET_ACTIVE_WINDOW` for a visible, non-minimized window) and `Minimized(false)`
//! sends it with `CurrentTime`, which KWin replaces with our own stale user time and
//! normally refuses.
//! `ViewportBuilder::with_taskbar` is Windows-only, so the taskbar entry is removed here too.

use anyhow::{Context as _, Result};
use eframe::CreationContext;
use winit::raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
use x11rb::connect;
use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::{
	Atom, AtomEnum, ClientMessageEvent, ConnectionExt as _, EventMask, PropMode, Window,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

/// EWMH source indication 2 ("pager"). KWin maps it to `NET::FromTool` and calls
/// `activateWindow(force=true)`; Mutter maps `CurrentTime` to "now".
/// Only ever sent in response to the user's shortcut.
const SOURCE_PAGER: u32 = 2;

/// The X11 window to activate. `Copy`, so the shortcut thread can own it.
#[derive(Clone, Copy)]
pub(crate) struct ActivateWindow(Window);

impl ActivateWindow {
	/// `None` when the window is not an X11 window (native Wayland).
	pub(crate) fn from_cc(cc: &CreationContext<'_>) -> Option<Self> {
		match cc.window_handle().ok()?.as_raw() {
			RawWindowHandle::Xlib(handle) => Some(Self(handle.window as Window)),
			_ => None,
		}
	}

	/// Remove the taskbar and Alt+Tab entries. `ViewportBuilder::with_taskbar` does
	/// nothing on X11.
	///
	/// Must run before the window is mapped (eframe maps it after the first frame), so
	/// the window manager reads the state when it starts managing the window.
	pub(crate) fn skip_taskbar(self) -> Result<()> {
		let (conn, _root) = connect_to_wm()?;
		let state = intern_atom(&conn, b"_NET_WM_STATE")?;
		let skip_taskbar = intern_atom(&conn, b"_NET_WM_STATE_SKIP_TASKBAR")?;
		// KWin's task switcher only honours the KDE atom; GNOME filters on SKIP_TASKBAR.
		let skip_switcher = intern_atom(&conn, b"_KDE_NET_WM_STATE_SKIP_SWITCHER")?;
		conn.change_property32(
			PropMode::REPLACE,
			self.0,
			state,
			AtomEnum::ATOM,
			&[skip_taskbar, skip_switcher],
		)?
		.check()
		.context("set _NET_WM_STATE")?;
		conn.flush().context("flush _NET_WM_STATE")
	}

	/// Raise, unminimize and focus. Call only from the user's shortcut.
	pub(crate) fn activate(self) -> Result<()> {
		let (conn, root) = connect_to_wm()?;
		let atom = intern_atom(&conn, b"_NET_ACTIVE_WINDOW")?;
		send_to_wm(&conn, root, activation_event(self.0, atom))
	}
}

// ── X11 connection ──

fn connect_to_wm() -> Result<(RustConnection, Window)> {
	let (conn, screen) = connect(None).context("connect to X11")?;
	let root = conn.setup().roots[screen].root;
	Ok((conn, root))
}

fn intern_atom(conn: &RustConnection, name: &[u8]) -> Result<Atom> {
	Ok(conn.intern_atom(false, name)?.reply()?.atom)
}

fn send_to_wm(conn: &RustConnection, root: Window, event: ClientMessageEvent) -> Result<()> {
	conn.send_event(false, root, EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY, event)?
		.check()
		.context("send message to window manager")?;
	conn.flush().context("flush message")
}

// ── Messages ──

/// `data = [source, timestamp, ...]`; timestamp 0 is `CurrentTime`.
fn activation_event(window: Window, atom: Atom) -> ClientMessageEvent {
	ClientMessageEvent::new(32, window, atom, [SOURCE_PAGER, 0, 0, 0, 0])
}

#[cfg(test)]
mod tests {
	use super::*;

	// ---- message layout ----

	#[test]
	fn activation_message_uses_pager_source_and_current_time() {
		let event = activation_event(42, 7);
		assert_eq!(event.window, 42);
		assert_eq!(event.type_, 7);
		assert_eq!(event.data.as_data32(), [SOURCE_PAGER, 0, 0, 0, 0]);
	}
}
