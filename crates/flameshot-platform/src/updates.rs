// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::time::Duration;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Update {
    pub version: String,
    pub url: String,
}
#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}
fn evaluate(bytes: &[u8], current: &str, ignored: &str) -> Result<Option<Update>> {
    let release: Release = serde_json::from_slice(bytes).context("Invalid release response")?;
    let version = semver::Version::parse(release.tag_name.trim_start_matches('v'))?;
    let installed = semver::Version::parse(current.trim_start_matches('v'))?;
    if release.draft || release.prerelease || !version.pre.is_empty() || version <= installed {
        return Ok(None);
    }
    if let Ok(ignore) = semver::Version::parse(ignored.trim_start_matches('v'))
        && version <= ignore
    {
        return Ok(None);
    }
    let url = url::Url::parse(&release.html_url)?;
    ensure!(
        url.scheme() == "https"
            && url.host_str() == Some("github.com")
            && url.path().starts_with("/flameshot-org/flameshot/releases/"),
        "Unexpected release link"
    );
    Ok(Some(Update {
        version: version.to_string(),
        url: release.html_url,
    }))
}
pub async fn check(current: &str, ignored: &str) -> Result<Option<Update>> {
    check_endpoint(
        "https://api.github.com/repos/flameshot-org/flameshot/releases/latest",
        current,
        ignored,
    )
    .await
}
async fn check_endpoint(endpoint: &str, current: &str, ignored: &str) -> Result<Option<Update>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("Flameshot-Rust/15.0.0")
        .build()?;
    let mut response = client
        .get(endpoint)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?
        .error_for_status()?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(
            bytes.len() + chunk.len() <= 1024 * 1024,
            "Release response too large"
        );
        bytes.extend_from_slice(&chunk);
    }
    evaluate(&bytes, current, ignored)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn release(version: &str) -> Vec<u8> {
        format!(r#"{{"tag_name":"{version}","html_url":"https://github.com/flameshot-org/flameshot/releases/tag/{version}"}}"#).into_bytes()
    }
    #[test]
    fn compares_versions_numerically_and_honors_ignored_releases() {
        assert_eq!(
            evaluate(&release("v15.10.0"), "15.9.0", "")
                .unwrap()
                .unwrap()
                .version,
            "15.10.0"
        );
        assert!(
            evaluate(&release("v15.0.0"), "15.0.0", "")
                .unwrap()
                .is_none()
        );
        assert!(
            evaluate(&release("v16.0.0-rc.1"), "15.0.0", "")
                .unwrap()
                .is_none()
        );
        assert!(
            evaluate(&release("v16.0.0"), "15.0.0", "v16.0.0")
                .unwrap()
                .is_none()
        );
        assert!(evaluate(b"{}", "15.0.0", "").is_err());
    }
    #[tokio::test]
    async fn fetches_releases_from_mock_http_service() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/latest", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = [0; 4096];
            let n = stream.read(&mut bytes).unwrap();
            assert!(String::from_utf8_lossy(&bytes[..n]).starts_with("GET /latest HTTP/1.1"));
            let body = release("v16.0.0");
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(&body).unwrap();
        });
        assert_eq!(
            check_endpoint(&endpoint, "15.0.0", "")
                .await
                .unwrap()
                .unwrap()
                .version,
            "16.0.0"
        );
        server.join().unwrap();
    }
}
