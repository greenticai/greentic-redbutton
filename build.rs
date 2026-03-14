use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let i18n_dir = manifest_dir.join("i18n");
    let locales_path = i18n_dir.join("locales.json");

    println!("cargo:rerun-if-changed={}", locales_path.display());

    let locales_raw = fs::read_to_string(&locales_path).expect("read locales.json");
    let locales: Vec<String> = serde_json::from_str(&locales_raw).expect("parse locales.json");

    let mut entries = Vec::new();
    for locale in &locales {
        let path = i18n_dir.join(format!("{locale}.json"));
        println!("cargo:rerun-if-changed={}", path.display());
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("missing locale file {}: {err}", path.display()));
        entries.push((locale.clone(), content));
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let generated = out_dir.join("i18n_bundle.rs");

    let supported = locales
        .iter()
        .map(|locale| format!("    {:?},", locale))
        .collect::<Vec<_>>()
        .join("\n");

    let embedded = entries
        .iter()
        .map(|(locale, content)| format!("    ({locale:?}, {content:?}),"))
        .collect::<Vec<_>>()
        .join("\n");

    let file = format!(
        "pub const SUPPORTED_LOCALES: &[&str] = &[\n{supported}\n];\n\npub const EMBEDDED_LOCALES: &[(&str, &str)] = &[\n{embedded}\n];\n"
    );

    fs::write(generated, file).expect("write generated i18n bundle");
}
