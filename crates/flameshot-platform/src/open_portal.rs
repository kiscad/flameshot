// SPDX-License-Identifier: GPL-3.0-or-later
//! OpenURI portal file-descriptor dispatch, including opening directories themselves.
use anyhow::{Context, Result, bail, ensure};
use futures_util::StreamExt;
use std::{
    collections::HashMap,
    fs::File,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use zbus::{
    Connection, Proxy,
    zvariant::{Fd, OwnedObjectPath, OwnedValue, Value},
};

const SERVICE: &str = "org.freedesktop.portal.Desktop";
const REQUEST: &str = "org.freedesktop.portal.Request";
static TOKEN: AtomicU64 = AtomicU64::new(1);

/// Returns false for user cancellation. `parent` may be empty for a tray action.
/// `ask` requests an application chooser; `writable` permits the chosen app to edit
/// the exported file rather than only read it.
/// OpenFile opens the supplied directory itself; OpenDirectory would reveal its
/// parent instead. Subscribe before calling to accept responses preceding the reply.
pub async fn open_file(
    connection: &Connection,
    parent: &str,
    file: &File,
    ask: bool,
    writable: bool,
    timeout: Duration,
) -> Result<bool> {
    let sender = connection
        .unique_name()
        .context("Portal connection has no bus name")?
        .as_str()
        .trim_start_matches(':')
        .replace('.', "_");
    let token = format!(
        "flameshot_open_{}_{}",
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
            "org.freedesktop.portal.OpenURI",
        )
        .await?;
        let options = HashMap::from([
            ("handle_token", Value::from(token.as_str())),
            ("ask", Value::from(ask)),
            ("writable", Value::from(writable)),
        ]);
        let returned: OwnedObjectPath = portal
            .call("OpenFile", &(parent, Fd::from(file), options))
            .await?;
        if returned.as_str() != path {
            close(connection, returned.as_str()).await;
            bail!("OpenURI portal ignored request handle token");
        }
        let message = responses
            .next()
            .await
            .context("OpenURI response stream ended")?;
        let (status, _): (u32, HashMap<String, OwnedValue>) = message.body().deserialize()?;
        match status {
            0 => Ok(true),
            1 => Ok(false),
            2 => bail!("OpenURI portal rejected the request"),
            _ => bail!("Invalid OpenURI response status {status}"),
        }
    };
    match tokio::time::timeout(timeout, operation).await {
        Ok(result) => result,
        Err(_) => {
            close(connection, &path).await;
            bail!("OpenURI portal timed out")
        }
    }
}

pub async fn open_directory(
    connection: &Connection,
    directory: &std::path::Path,
    timeout: Duration,
) -> Result<bool> {
    ensure!(
        directory.is_dir(),
        "Save path is not a directory: {}",
        directory.display()
    );
    let file = File::open(directory)
        .with_context(|| format!("Cannot open save directory: {}", directory.display()))?;
    ensure!(
        file.metadata()?.is_dir(),
        "Save path is not a directory: {}",
        directory.display()
    );
    open_file(connection, "", &file, false, false, timeout).await
}

async fn close(connection: &Connection, path: &str) {
    let _ = tokio::time::timeout(Duration::from_secs(1), async {
        Proxy::new(connection, SERVICE, path, REQUEST)
            .await?
            .call::<_, _, ()>("Close", &())
            .await
    })
    .await;
}
