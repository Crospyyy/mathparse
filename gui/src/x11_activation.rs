//! Unminimize + focus our window through the window manager.
//!
//! `ViewportCommand::Focus` does nothing while the window is minimized (winit only
//! sends `_NET_ACTIVE_WINDOW` for a visible window) and `Minimized(false)` sends it
//! with `CurrentTime`, which KWin replaces with our own stale user time and refuses.

use anyhow::{Context as _, Result};
use eframe::CreationContext;
use winit::raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
use x11rb::connect;
use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::{Atom, ClientMessageEvent, ConnectionExt as _, EventMask, Window};

/// EWMH source indication 2 ("pager"). KWin maps it to `NET::FromTool` and calls
/// `activateWindow(window, force=true)`; Mutter maps `CurrentTime` to "now".
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

	/// Raise, unminimize and focus. Call only from the user's shortcut.
	pub(crate) fn activate(self) -> Result<()> {
		let (conn, screen) = connect(None).context("connect to X11")?;
		let root = conn.setup().roots[screen].root;
		let atom = conn
			.intern_atom(false, b"_NET_ACTIVE_WINDOW")?
			.reply()
			.context("intern _NET_ACTIVE_WINDOW")?
			.atom;

		conn.send_event(
			false,
			root,
			EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
			activation_event(self.0, atom),
		)?
		.check()
		.context("send _NET_ACTIVE_WINDOW")?;
		conn.flush().context("flush activation")
	}
}

// ── Message ──

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
