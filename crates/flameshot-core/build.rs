// SPDX-License-Identifier: GPL-3.0-or-later
use std::{collections::BTreeMap, env, fs, path::PathBuf};
fn main() {
    let directory =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../../data/translations");
    println!("cargo:rerun-if-changed={}", directory.display());
    let mut catalogs = BTreeMap::new();
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) != Some("ts") {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap();
        let xml = roxmltree::Document::parse_with_options(
            &text,
            roxmltree::ParsingOptions {
                allow_dtd: true,
                ..Default::default()
            },
        )
        .unwrap();
        let language = path
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .strip_prefix("Internationalization_")
            .unwrap()
            .to_owned();
        let mut messages = BTreeMap::new();
        for context in xml.descendants().filter(|n| n.has_tag_name("context")) {
            let name = context
                .children()
                .find(|n| n.has_tag_name("name"))
                .and_then(|n| n.text())
                .unwrap_or("");
            for message in context.children().filter(|n| n.has_tag_name("message")) {
                let source = message
                    .children()
                    .find(|n| n.has_tag_name("source"))
                    .and_then(|n| n.text())
                    .unwrap_or("");
                let Some(translation) = message.children().find(|n| n.has_tag_name("translation"))
                else {
                    continue;
                };
                if matches!(
                    translation.attribute("type"),
                    Some("unfinished" | "vanished" | "obsolete")
                ) {
                    continue;
                }
                if let Some(value) = translation.text().filter(|s| !s.is_empty()) {
                    messages.insert((name.to_owned(), source.to_owned()), value.to_owned());
                }
            }
        }
        catalogs.insert(language, messages);
    }
    let mut output = String::from("static CATALOGS: &[Catalog] = &[\n");
    for (language, messages) in catalogs {
        output.push_str(&format!(
            "Catalog {{ language: {language:?}, messages: &[\n"
        ));
        for ((context, source), translation) in messages {
            output.push_str(&format!("({context:?},{source:?},{translation:?}),\n"));
        }
        output.push_str("] },\n");
    }
    output.push_str("];\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("catalogs.rs"),
        output,
    )
    .unwrap();
}
