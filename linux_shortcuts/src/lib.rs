use std::{
	str::FromStr,
	thread::{self, JoinHandle},
};

use ashpd::{
	AppID,
	desktop::{
		CreateSessionOptions,
		global_shortcuts::{BindShortcutsOptions, GlobalShortcuts, NewShortcut},
	},
	register_host_app,
};
use futures::{StreamExt, executor::block_on};

pub fn non_async_global_shortcuts(shortcut_action: impl Fn() + 'static + Send) -> JoinHandle<()> {
	thread::spawn(|| block_on(global_shortcuts(shortcut_action)))
}

async fn global_shortcuts(shortcut_action: impl Fn() + 'static + Send) {
	let app_id = AppID::from_str("io.github.Crospyyy.mathparse").expect("app id is a valid constant");
	if let Err(err) = register_host_app(app_id).await {
		eprintln!("host app registration failed: {err}");
	}
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
