// SPDX-License-Identifier: GPL-3.0-or-later
//! Screenshot portal request lifecycle. Subscribe before calling Screenshot so
//! synchronous/fast portal responses cannot race signal registration.
use futures_util::StreamExt;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use zbus::{
    Connection, Proxy,
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};

static TOKEN: AtomicU64 = AtomicU64::new(1);
const SERVICE: &str = "org.freedesktop.portal.Desktop";
const REQUEST: &str = "org.freedesktop.portal.Request";

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("Screenshot cancelled")]
    Cancelled,
    #[error("Screenshot portal timed out")]
    Timeout,
    #[error("Screenshot portal rejected the request")]
    Rejected,
    #[error("Screenshot portal: {0}")]
    Bus(#[from] zbus::Error),
    #[error("Invalid screenshot portal response: {0}")]
    InvalidResponse(String),
}

/// Return the portal-provided local file. The caller owns decoding and cleanup
/// policy: a portal URI is not necessarily an exclusively owned temporary file.
/// `parent` must be an exported native window identifier (or empty if absent).
pub async fn screenshot(
    connection: &Connection,
    parent: &str,
    timeout: Duration,
) -> Result<PathBuf, CaptureError> {
    let sender = connection
        .unique_name()
        .ok_or_else(|| CaptureError::InvalidResponse("Connection has no bus name".into()))?
        .as_str()
        .trim_start_matches(':')
        .replace('.', "_");
    let token = format!(
        "flameshot_{}_{}",
        std::process::id(),
        TOKEN.fetch_add(1, Ordering::Relaxed)
    );
    let path = format!("/org/freedesktop/portal/desktop/request/{sender}/{token}");
    let operation = async {
        let request = Proxy::new(connection, SERVICE, path.as_str(), REQUEST).await?;
        let mut responses = request.receive_signal("Response").await?;
        let portal = Proxy::new(
            connection,
            SERVICE,
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Screenshot",
        )
        .await?;
        let options = HashMap::from([
            ("handle_token", Value::from(token.as_str())),
            ("interactive", Value::from(false)),
        ]);
        let returned: OwnedObjectPath = portal.call("Screenshot", &(parent, options)).await?;
        if returned.as_str() != path {
            // Modern portals honor handle_token. Do not silently listen to the
            // wrong object if a broken backend returns a different path.
            close(connection, returned.as_str()).await;
            return Err(CaptureError::InvalidResponse(
                "Portal ignored request handle token".into(),
            ));
        }
        let message = responses
            .next()
            .await
            .ok_or_else(|| CaptureError::InvalidResponse("Response stream ended".into()))?;
        let (status, results): (u32, HashMap<String, OwnedValue>) = message.body().deserialize()?;
        match status {
            0 => {
                let uri = results
                    .get("uri")
                    .and_then(|value| <&str>::try_from(value).ok())
                    .ok_or_else(|| {
                        CaptureError::InvalidResponse("Missing screenshot URI".into())
                    })?;
                local_path(uri)
            }
            1 => Err(CaptureError::Cancelled),
            2 => Err(CaptureError::Rejected),
            _ => Err(CaptureError::InvalidResponse(format!(
                "Unknown response status {status}"
            ))),
        }
    };
    let result = tokio::time::timeout(timeout, operation).await;
    match result {
        Ok(result) => result,
        Err(_) => {
            close(connection, &path).await;
            Err(CaptureError::Timeout)
        }
    }
}

async fn close(connection: &Connection, path: &str) {
    // Cleanup must not turn a timeout into another unbounded wait.
    let _ = tokio::time::timeout(Duration::from_secs(1), async {
        let request = Proxy::new(connection, SERVICE, path, REQUEST).await?;
        request.call::<_, _, ()>("Close", &()).await
    })
    .await;
}

fn local_path(uri: &str) -> Result<PathBuf, CaptureError> {
    url::Url::parse(uri)
        .ok()
        .and_then(|uri| uri.to_file_path().ok())
        .ok_or_else(|| CaptureError::InvalidResponse("Screenshot URI is not a local file".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decodes_local_unicode_path_without_accepting_remote_uris() {
        assert_eq!(
            local_path("file:///tmp/%E6%88%AA%E5%9B%BE%20one.png").unwrap(),
            PathBuf::from("/tmp/截图 one.png")
        );
        assert!(local_path("https://example.org/screenshot.png").is_err());
        assert!(local_path("file://remote/tmp/screenshot.png").is_err());
        assert!(local_path("not a uri").is_err());
    }
}
