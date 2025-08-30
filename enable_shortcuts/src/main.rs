use std::sync::atomic::{AtomicBool, Ordering};

fn main() {
	use rdev::{grab, Event, EventType, Key};

	let alt_pressed = AtomicBool::new(false);

	let callback = move |event: Event| -> Option<Event> {
		match event.event_type {
			EventType::KeyPress(Key::Alt) => {
				alt_pressed.store(true, Ordering::Relaxed);
				println!("Alt pressed");
			}
			EventType::KeyRelease(Key::Alt) => {
				alt_pressed.store(false, Ordering::Relaxed);
				println!("Alt released");
			}
			EventType::KeyPress(Key::Space) => {
				println!("Space pressed");
				if alt_pressed.load(Ordering::Relaxed) {
					println!("Alt+Space detected!");
					return None;
				}
			}
			_ => {}
		}
		Some(event)
	};
	// This will block.
	if let Err(error) = grab(callback) {
		println!("Error: {:?}", error)
	}
}
