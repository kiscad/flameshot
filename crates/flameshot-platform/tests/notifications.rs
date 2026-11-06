// SPDX-License-Identifier: GPL-3.0-or-later
use std::collections::HashMap;
use zbus::zvariant::OwnedValue;
struct Notifications;
#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Notifications {
    #[allow(clippy::too_many_arguments)] // Freedesktop Notify wire signature.
    fn notify(
        &self,
        app: &str,
        replaces: u32,
        icon: &str,
        title: &str,
        body: &str,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        timeout: i32,
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
    let _server = zbus::connection::Builder::session()
        .unwrap()
        .name("org.freedesktop.Notifications")
        .unwrap()
        .serve_at("/org/freedesktop/Notifications", Notifications)
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
}
