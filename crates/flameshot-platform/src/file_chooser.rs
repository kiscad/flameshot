// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::{Context, Result, bail, ensure};
use futures_util::StreamExt;
use std::{
    collections::HashMap,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::Duration,
};
use zbus::{
    Connection, Proxy,
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};
static TOKEN: AtomicU64 = AtomicU64::new(1);
const SERVICE: &str = "org.freedesktop.portal.Desktop";
const REQUEST: &str = "org.freedesktop.portal.Request";
type Filter = (String, Vec<(u32, String)>);
#[derive(Debug)]
pub struct SaveSelection {
    pub path: PathBuf,
    pub extension: String,
}

/// A successful selection authorizes saving to this exact path, including replacement.
/// Do not append a suffix or collision counter to a document-portal filename.
pub async fn save_file(
    connection: &Connection,
    parent: &str,
    title: &str,
    suggested: &Path,
    extensions: &[&str],
    timeout: Duration,
) -> Result<Option<SaveSelection>> {
    save_file_cancellable(
        connection,
        parent,
        title,
        suggested,
        extensions,
        timeout,
        std::future::pending(),
    )
    .await
}

/// Cancel without accepting a late selection. If SaveFile is still replying,
/// Close is attempted immediately and the eventual handle is cleaned up too.
/// The original timeout bounds cleanup. Signal cancellation rather than dropping
/// this future; UI callers can leave it running in a worker after dismissing UI.
pub async fn save_file_cancellable(
    connection: &Connection,
    parent: &str,
    title: &str,
    suggested: &Path,
    extensions: &[&str],
    timeout: Duration,
    cancel: impl std::future::Future<Output = ()>,
) -> Result<Option<SaveSelection>> {
    tokio::pin!(cancel);
    let mut cancelled = false;
    ensure!(!extensions.is_empty(), "No supported save formats");
    let suggested = std::path::absolute(suggested)?;
    let name = suggested
        .file_name()
        .context("Missing suggested filename")?
        .to_string_lossy();
    let mut folder = suggested
        .parent()
        .context("Missing suggested folder")?
        .as_os_str()
        .as_bytes()
        .to_vec();
    folder.push(0);
    let filters: Vec<Filter> = extensions
        .iter()
        .map(|ext| {
            let pattern = format!(
                "*.{}",
                ext.chars()
                    .map(|c| format!("[{}{}]", c.to_ascii_lowercase(), c.to_ascii_uppercase()))
                    .collect::<String>()
            );
            (ext.to_uppercase(), vec![(0, pattern)])
        })
        .collect();
    let default_index = extensions
        .iter()
        .position(|ext| {
            suggested
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case(ext))
        })
        .unwrap_or(0);
    let sender = connection
        .unique_name()
        .context("Portal connection has no bus name")?
        .as_str()
        .trim_start_matches(':')
        .replace('.', "_");
    let token = format!(
        "flameshot_save_{}_{}",
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
            "org.freedesktop.portal.FileChooser",
        )
        .await?;
        let options = HashMap::from([
            ("handle_token", Value::from(token.as_str())),
            ("current_name", Value::from(name.as_ref())),
            ("current_folder", Value::from(folder)),
            ("filters", Value::from(filters.clone())),
            (
                "current_filter",
                Value::from(filters[default_index].clone()),
            ),
            ("modal", Value::from(true)),
        ]);
        let started = AtomicBool::new(false);
        let call = async {
            started.store(true, Ordering::Relaxed);
            portal
                .call::<_, _, OwnedObjectPath>("SaveFile", &(parent, title, options))
                .await
        };
        tokio::pin!(call);
        let returned = tokio::select! {
            biased;
            _ = &mut cancel => {
                cancelled = true;
                if !started.load(Ordering::Relaxed) { return Ok(None); }
                let closed = close(connection, &path).await;
                match call.await {
                    Ok(returned) if !closed || returned.as_str() != path => { close(connection, returned.as_str()).await; }
                    Err(_) if !closed => { close(connection, &path).await; }
                    _ => {}
                }
                return Ok(None);
            }
            returned = &mut call => returned?,
        };
        if returned.as_str() != path {
            close(connection, returned.as_str()).await;
            bail!("FileChooser ignored request handle token");
        }
        let message = tokio::select! {
            biased;
            _ = &mut cancel => {
                cancelled = true;
                close(connection, &path).await;
                return Ok(None);
            }
            message = responses.next() => message.context("FileChooser response stream ended")?,
        };
        let (status, mut results): (u32, HashMap<String, OwnedValue>) =
            message.body().deserialize()?;
        match status {
            0 => {
                let uris: Vec<String> = results
                    .remove("uris")
                    .context("Missing save URI")?
                    .try_into()?;
                ensure!(uris.len() == 1, "SaveFile must return exactly one URI");
                let uri = url::Url::parse(&uris[0])?;
                ensure!(
                    uri.query().is_none() && uri.fragment().is_none(),
                    "Save URI contains a query or fragment"
                );
                let selected = uri
                    .to_file_path()
                    .map_err(|_| anyhow::anyhow!("Save URI is not a local file"))?;
                ensure!(
                    selected.is_absolute() && selected.file_name().is_some(),
                    "Invalid save path"
                );
                let filter: Option<Filter> = results
                    .remove("current_filter")
                    .map(TryInto::try_into)
                    .transpose()?;
                let index = filter
                    .and_then(|filter| filters.iter().position(|candidate| *candidate == filter))
                    .unwrap_or(default_index);
                Ok(Some(SaveSelection {
                    path: selected,
                    extension: extensions[index].into(),
                }))
            }
            1 => Ok(None),
            2 => bail!("FileChooser rejected the request"),
            _ => bail!("Invalid FileChooser response status {status}"),
        }
    };
    match tokio::time::timeout(timeout, operation).await {
        Ok(result) => result,
        Err(_) => {
            close(connection, &path).await;
            if cancelled {
                Ok(None)
            } else {
                bail!("FileChooser timed out")
            }
        }
    }
}
async fn close(connection: &Connection, path: &str) -> bool {
    matches!(
        tokio::time::timeout(Duration::from_secs(1), async {
            Proxy::new(connection, SERVICE, path, REQUEST)
                .await?
                .call::<_, _, ()>("Close", &())
                .await
        })
        .await,
        Ok(Ok(()))
    )
}
