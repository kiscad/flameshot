// SPDX-License-Identifier: GPL-3.0-or-later
use std::collections::HashMap;
use zbus::zvariant::OwnedValue;
struct Notifications {
    received: tokio::sync::mpsc::UnboundedSender<()>,
    gate: std::sync::Arc<tokio::sync::Semaphore>,
}
#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Notifications {
    #[allow(clippy::too_many_arguments)] // Freedesktop Notify wire signature.
    async fn notify(
        &self,
        app: &str,
        replaces: u32,
        icon: &str,
        title: &str,
        body: &str,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        timeout: i32,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> u32 {
        assert_eq!(app, "Flameshot");
        assert_eq!(replaces, 0);
        assert_eq!(icon, "org.flameshot.Flameshot");
        assert_eq!(title, "Saved");
        assert_eq!(body, "截图.png");
        assert!(actions.is_empty());
        assert_eq!(timeout, -1);
        assert_eq!(
            <&str>::try_from(hints.get("desktop-entry").unwrap()).unwrap(),
            "org.flameshot.Flameshot"
        );
        if header
            .primary()
            .flags()
            .contains(zbus::message::Flags::NoReplyExpected)
        {
            self.received.send(()).unwrap();
            self.gate.acquire().await.unwrap().forget();
        }
        42
    }
}
#[tokio::test]
#[ignore = "requires private D-Bus"]
async fn notifications_use_standard_wire_signature() {
    assert_eq!(
        std::env::var("FLAMESHOT_TEST_PRIVATE_BUS").as_deref(),
        Ok("1")
    );
    let (sender, mut received) = tokio::sync::mpsc::unbounded_channel();
    let gate = std::sync::Arc::new(tokio::sync::Semaphore::new(0));
    let _server = zbus::connection::Builder::session()
        .unwrap()
        .name("org.freedesktop.Notifications")
        .unwrap()
        .serve_at(
            "/org/freedesktop/Notifications",
            Notifications {
                received: sender,
                gate: gate.clone(),
            },
        )
        .unwrap()
        .build()
        .await
        .unwrap();
    let client = zbus::Connection::session().await.unwrap();
    assert_eq!(
        flameshot_platform::desktop::notify(&client, "Saved", "截图.png")
            .await
            .unwrap(),
        42
    );
    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        flameshot_platform::desktop::notify_no_reply(&client, "Saved", "截图.png"),
    )
    .await
    .expect("No-reply notification waited for the server")
    .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(1), received.recv())
        .await
        .unwrap()
        .unwrap();
    gate.add_permits(1);
}
