use rdev::{Event, EventType, GrabError, grab};
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::thread::JoinHandle;

pub use rdev::Key;

static REGISTERED: LazyLock<AtomicBool> = LazyLock::new(|| AtomicBool::new(false));
static MODIFIER_STATE: LazyLock<ModifierState> = LazyLock::new(|| ModifierState::new());

#[derive(Debug)]
struct ModifierState {
    alt: AtomicBool,
    shift_l: AtomicBool,
    shift_r: AtomicBool,
    ctrl_l: AtomicBool,
    ctrl_r: AtomicBool,
}

impl ModifierState {
    fn new() -> Self {
        Self {
            alt: AtomicBool::new(false),
            shift_l: AtomicBool::new(false),
            shift_r: AtomicBool::new(false),
            ctrl_l: AtomicBool::new(false),
            ctrl_r: AtomicBool::new(false),
        }
    }

    fn modifier_match(&self, modifiers: Modifiers) -> bool {
        (modifiers.alt == self.alt.load(Ordering::Relaxed))
            && (modifiers.shift == self.shift_l.load(Ordering::Relaxed)
                || modifiers.shift == self.shift_r.load(Ordering::Relaxed))
            && (modifiers.ctrl == self.ctrl_l.load(Ordering::Relaxed)
                || modifiers.ctrl == self.ctrl_r.load(Ordering::Relaxed))
    }
}

#[derive(Eq, PartialEq, Hash, Copy, Clone)]
pub struct Modifiers {
    alt: bool,
    shift: bool,
    ctrl: bool,
}

impl Modifiers {
    pub fn new(alt: bool, shift: bool, ctrl: bool) -> Self {
        Self { alt, shift, ctrl }
    }
    pub const ALT: Self = Self { alt: true, shift: false, ctrl: false };
    pub const SHIFT: Self = Self { alt: false, shift: true, ctrl: false };
    pub const CTRL: Self = Self { alt: false, shift: false, ctrl: true };
}

pub fn register_global_shortcut(
    check_modifiers: Modifiers, trigger_key: Key, shortcut_action: impl Fn() + 'static + Send,
) -> Option<JoinHandle<Result<(), Option<GrabError>>>> {
    if REGISTERED.load(Ordering::Relaxed) {
        return None;
    } else {
        REGISTERED.store(true, Ordering::Relaxed);
    }

    let callback = move |event: Event| -> Option<Event> {
        macro_rules! generate_match {
            ($ident:ident) => {
                MODIFIER_STATE
                    .$ident
                    .store(matches!(event.event_type, EventType::KeyPress(_)), Ordering::Relaxed)
            };
            ($pat:pat) => {
                EventType::KeyPress($pat) | EventType::KeyRelease($pat)
            };
        }

        match event.event_type {
            generate_match!(Key::Alt) => generate_match!(alt),
            generate_match!(Key::ControlLeft) => generate_match!(ctrl_l),
            generate_match!(Key::ControlRight) => generate_match!(ctrl_r),
            generate_match!(Key::ShiftLeft) => generate_match!(shift_l),
            generate_match!(Key::ShiftRight) => generate_match!(shift_r),

            EventType::KeyPress(key) if key == trigger_key => {
                if MODIFIER_STATE.modifier_match(check_modifiers) {
                    shortcut_action();
                    return None; // capture event
                }
            },
            _ => {},
        }

        Some(event)
    };

    Some(thread::spawn(move || {
        // This will block.
        if let Err(error) = grab(callback) {
            return Err(Some(error));
        };
        return Ok(());
    }))
}
