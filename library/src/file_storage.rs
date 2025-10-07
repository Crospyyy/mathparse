#[test]
fn write_toml() {
	let t = toml::toml! { key = { inner = "value" } };
	println!("{}", t.get("key").unwrap());
}
