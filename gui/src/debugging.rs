#[macro_export]
macro_rules! log {
	($tt:tt) => {{
		eprint!("[{}:{}:{}] ", file!(), line!(), column!());
		println!($tt);
	}};
}
