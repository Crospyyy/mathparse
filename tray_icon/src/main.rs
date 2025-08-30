use tray_icon::menu::MenuItem;

fn main() {
	use tray_icon::{TrayIconBuilder, menu::Menu};

	let tray_menu = Menu::new();
	tray_menu.append(&MenuItem::new("Some Item", true, None)).unwrap();
	let tray_icon = TrayIconBuilder::new()
		.with_menu(Box::new(tray_menu))
		.with_tooltip("system-tray - tray icon library!")
		.build()
		.unwrap();
	tray_icon.set_visible(true).unwrap();
	loop {}
}
