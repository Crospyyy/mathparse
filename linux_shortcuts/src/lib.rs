use std::thread::{self, JoinHandle};

use ashpd::desktop::{
	CreateSessionOptions,
	global_shortcuts::{BindShortcutsOptions, GlobalShortcuts, NewShortcut},
};
use futures::{StreamExt, executor::block_on};

pub fn non_async_global_shortcuts(shortcut_action: impl Fn() + 'static + Send) -> JoinHandle<()> {
	thread::spawn(|| block_on(global_shortcuts(shortcut_action)))
}

async fn global_shortcuts(shortcut_action: impl Fn() + 'static + Send) {
	let shortcuts = GlobalShortcuts::new().await.unwrap();
	let session = shortcuts.create_session(CreateSessionOptions::default()).await.unwrap();
	let idk = shortcuts
		.bind_shortcuts(
			&session,
			&[NewShortcut::new("open-calculator", "Open the calculator")],
			None,
			BindShortcutsOptions::default(),
		)
		.await
		.unwrap();
	let resp = idk.response();
	println!("{:?}", resp);
	let idk = shortcuts.receive_activated().await.unwrap();
	futures::pin_mut!(idk);
	while let Some(activated) = idk.next().await {
		println!("Wert empfangen: {:?}", activated);
		shortcut_action();
	}
}
