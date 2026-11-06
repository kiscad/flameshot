// SPDX-License-Identifier: GPL-3.0-or-later
use std::{collections::HashMap, process::Command};
use zbus::{
    Connection,
    message::Header,
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};

struct Portal(String);
#[zbus::interface(name = "org.freedesktop.portal.Screenshot")]
impl Portal {
    async fn screenshot(
        &self,
        _parent: &str,
        options: HashMap<String, OwnedValue>,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> OwnedObjectPath {
        let sender = header
            .sender()
            .unwrap()
            .as_str()
            .trim_start_matches(':')
            .replace('.', "_");
        let token = <&str>::try_from(options.get("handle_token").unwrap()).unwrap();
        let path: OwnedObjectPath =
            format!("/org/freedesktop/portal/desktop/request/{sender}/{token}")
                .try_into()
                .unwrap();
        connection
            .emit_signal(
                header.sender().map(|name| name.as_str()),
                path.clone(),
                "org.freedesktop.portal.Request",
                "Response",
                &(
                    0_u32,
                    HashMap::from([("uri", Value::from(self.0.as_str()))]),
                ),
            )
            .await
            .unwrap();
        path
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires private D-Bus"]
async fn full_outputs_only_png_on_stdout_and_saves_without_overwrite() {
    assert_eq!(
        std::env::var("FLAMESHOT_TEST_PRIVATE_BUS").as_deref(),
        Ok("1")
    );
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.png");
    let expected = image::RgbaImage::from_fn(9, 7, |x, y| {
        image::Rgba([x as u8 * 20, y as u8 * 30, 150, 255])
    });
    expected.save(&source).unwrap();
    let _service = zbus::connection::Builder::session()
        .unwrap()
        .name("org.freedesktop.portal.Desktop")
        .unwrap()
        .serve_at(
            "/org/freedesktop/portal/desktop",
            Portal(format!("file://{}", source.display())),
        )
        .unwrap()
        .build()
        .await
        .unwrap();
    let destination = temp.path().join("capture.png");
    for _ in 0..2 {
        let result = Command::new(env!("CARGO_BIN_EXE_flameshot"))
            .env("XDG_CONFIG_HOME", temp.path().join("config"))
            .args(["full", "--raw", "--path"])
            .arg(&destination)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            image::load_from_memory(&result.stdout).unwrap().to_rgba8(),
            expected
        );
    }
    for filename in ["capture.png", "capture_1.png"] {
        assert_eq!(
            image::open(temp.path().join(filename)).unwrap().to_rgba8(),
            expected
        );
    }
    assert!(
        source.exists(),
        "Portal-provided file must not be blindly deleted"
    );
}
