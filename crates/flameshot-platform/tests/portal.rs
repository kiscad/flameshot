// SPDX-License-Identifier: GPL-3.0-or-later
use flameshot_platform::portal::{CaptureError, screenshot};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};
use zbus::{
    Connection,
    message::Header,
    zvariant::{OwnedObjectPath, OwnedValue},
};

struct MockPortal {
    status: Arc<AtomicU32>,
    closed: Arc<AtomicU32>,
}
struct MockRequest(Arc<AtomicU32>);
#[zbus::interface(name = "org.freedesktop.portal.Request")]
impl MockRequest {
    fn close(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
#[zbus::interface(name = "org.freedesktop.portal.Screenshot")]
impl MockPortal {
    async fn screenshot(
        &self,
        parent: &str,
        options: HashMap<String, OwnedValue>,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        assert_eq!(parent, "x11:123");
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
            .object_server()
            .at(path.clone(), MockRequest(self.closed.clone()))
            .await
            .unwrap();
        let status = self.status.load(Ordering::SeqCst);
        if status != 3 {
            let results = HashMap::from([(
                "uri",
                zbus::zvariant::Value::from("file:///tmp/portal%20test.png"),
            )]);
            // Deliberately emit before returning Screenshot's method reply.
            connection
                .emit_signal(
                    header.sender().map(|name| name.as_str()),
                    path.clone(),
                    "org.freedesktop.portal.Request",
                    "Response",
                    &(status, results),
                )
                .await
                .unwrap();
        }
        Ok(path)
    }
}

#[tokio::test]
#[ignore = "requires dedicated dbus-run-session and FLAMESHOT_TEST_PRIVATE_BUS=1"]
async fn portal_lifecycle() {
    assert_eq!(
        std::env::var("FLAMESHOT_TEST_PRIVATE_BUS").as_deref(),
        Ok("1")
    );
    let status = Arc::new(AtomicU32::new(0));
    let closed = Arc::new(AtomicU32::new(0));
    let _server = zbus::connection::Builder::session()
        .unwrap()
        .name("org.freedesktop.portal.Desktop")
        .unwrap()
        .serve_at(
            "/org/freedesktop/portal/desktop",
            MockPortal {
                status: status.clone(),
                closed: closed.clone(),
            },
        )
        .unwrap()
        .build()
        .await
        .unwrap();
    let client = Connection::session().await.unwrap();
    let timeout = Duration::from_secs(2);
    assert_eq!(
        screenshot(&client, "x11:123", timeout)
            .await
            .unwrap()
            .to_str(),
        Some("/tmp/portal test.png")
    );
    status.store(1, Ordering::SeqCst);
    assert!(matches!(
        screenshot(&client, "x11:123", timeout).await,
        Err(CaptureError::Cancelled)
    ));
    status.store(2, Ordering::SeqCst);
    assert!(matches!(
        screenshot(&client, "x11:123", timeout).await,
        Err(CaptureError::Rejected)
    ));
    status.store(3, Ordering::SeqCst);
    assert!(matches!(
        screenshot(&client, "x11:123", Duration::from_millis(100)).await,
        Err(CaptureError::Timeout)
    ));
    assert_eq!(closed.load(Ordering::SeqCst), 1);
}
