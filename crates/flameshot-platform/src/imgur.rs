// SPDX-License-Identifier: GPL-3.0-or-later
//! Anonymous Imgur upload/delete transport. No requests occur until explicitly called.
use anyhow::{Context, Result, ensure};
use reqwest::{
    Client,
    header::{AUTHORIZATION, HeaderValue},
    multipart::{Form, Part},
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadedImage {
    pub id: String,
    pub link: String,
    pub deletehash: String,
}
pub struct Imgur {
    client: Client,
    authorization: HeaderValue,
    endpoint: String,
}
impl Imgur {
    pub fn new(client_id: &str) -> Result<Self> {
        ensure!(!client_id.is_empty(), "An Imgur Client-ID is required");
        let mut authorization = HeaderValue::from_str(&format!("Client-ID {client_id}"))?;
        authorization.set_sensitive(true);
        Ok(Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .connect_timeout(Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .user_agent("Flameshot-Rust/15.0.0")
                .build()?,
            authorization,
            endpoint: "https://api.imgur.com/3/image".into(),
        })
    }
    /// Dependency injection for local integration tests; never accepts remote hosts.
    #[doc(hidden)]
    pub fn local_test_server(client_id: &str, endpoint: &str) -> Result<Self> {
        let url = reqwest::Url::parse(endpoint)?;
        ensure!(
            url.scheme() == "http"
                && url.host_str().is_some_and(|host| host
                    .trim_matches(['[', ']'])
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback()))
                && url.port().is_some()
                && url.username().is_empty()
                && url.password().is_none(),
            "Test endpoint must be an explicit loopback HTTP address"
        );
        let mut api = Self::new(client_id)?;
        api.client = Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("Flameshot-Rust/15.0.0")
            .build()?;
        api.endpoint = url.to_string();
        Ok(api)
    }
    pub async fn upload(&self, png: Vec<u8>, description: &str) -> Result<UploadedImage> {
        ensure!(
            png.starts_with(b"\x89PNG\r\n\x1a\n"),
            "Upload data must be a PNG image"
        );
        let form = Form::new()
            .part(
                "image",
                Part::bytes(png)
                    .file_name("capture.png")
                    .mime_str("image/png")?,
            )
            .text("type", "file")
            .text("description", description.to_owned());
        let response = self
            .client
            .post(&self.endpoint)
            .header(AUTHORIZATION, self.authorization.clone())
            .multipart(form)
            .send()
            .await
            .map_err(reqwest::Error::without_url)?;
        let response = read_response(response).await?;
        ensure!(
            response.get("success").and_then(|v| v.as_bool()) == Some(true),
            "Imgur did not confirm upload success"
        );
        let uploaded: UploadedImage = serde_json::from_value(
            response
                .get("data")
                .cloned()
                .context("Missing Imgur image data")?,
        )?;
        ensure!(
            valid_token(&uploaded.id) && valid_token(&uploaded.deletehash),
            "Invalid Imgur image identifiers"
        );
        let url = url::Url::parse(&uploaded.link)?;
        ensure!(
            url.scheme() == "https" && matches!(url.host_str(), Some("i.imgur.com" | "imgur.com")),
            "Invalid Imgur image link"
        );
        Ok(uploaded)
    }
    pub async fn delete(&self, deletehash: &str) -> Result<()> {
        ensure!(valid_token(deletehash), "Invalid Imgur deletion token");
        let response = self
            .client
            .delete(format!("{}/{}", self.endpoint, deletehash))
            .header(AUTHORIZATION, self.authorization.clone())
            .send()
            .await
            .map_err(reqwest::Error::without_url)?;
        let response = read_response(response).await?;
        ensure!(
            response.get("success").and_then(|v| v.as_bool()) == Some(true)
                && response.get("data").and_then(|v| v.as_bool()) == Some(true),
            "Imgur did not confirm deletion"
        );
        Ok(())
    }
}
fn valid_token(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && value.bytes().all(|b| b.is_ascii_alphanumeric())
}
async fn read_response(mut response: reqwest::Response) -> Result<serde_json::Value> {
    ensure!(
        response.status().is_success(),
        "Imgur request failed: HTTP {}",
        response.status().as_u16()
    );
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(reqwest::Error::without_url)?
    {
        ensure!(
            bytes.len() + chunk.len() <= 2 * 1024 * 1024,
            "Imgur response is too large"
        );
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).context("Invalid Imgur JSON response")
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::mpsc,
        thread,
    };
    fn server(
        status: &'static str,
        body: &'static str,
    ) -> (String, mpsc::Receiver<Vec<u8>>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/3/image", listener.local_addr().unwrap());
        let (sender, receiver) = mpsc::channel();
        let task = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0; 4096];
            loop {
                let read = stream.read(&mut buffer).unwrap();
                assert!(read > 0);
                bytes.extend_from_slice(&buffer[..read]);
                if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&bytes[..end]);
                    let length = header
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if bytes.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            sender.send(bytes).unwrap();
            write!(stream,"HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        });
        (endpoint, receiver, task)
    }
    #[tokio::test]
    async fn upload_sends_png_and_reads_anonymous_deletion_token() {
        let (endpoint, request, task) = server(
            "200 OK",
            r#"{"success":true,"data":{"id":"abc123","link":"https://i.imgur.com/abc123.png","deletehash":"secret123"}}"#,
        );
        let mut api = Imgur::new("test-client").unwrap();
        api.endpoint = endpoint;
        let png =
            include_bytes!("../../flameshot-core/tests/fixtures/qt6-clipboard.bin")[4..].to_vec();
        let uploaded = api.upload(png.clone(), "synthetic test").await.unwrap();
        assert_eq!(uploaded.deletehash, "secret123");
        let bytes = request.recv().unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.starts_with("POST /3/image HTTP/1.1"));
        assert!(text.contains("Client-ID test-client"));
        assert!(text.contains("name=\"image\""));
        assert!(bytes.windows(png.len()).any(|w| w == png));
        task.join().unwrap();
    }
    #[tokio::test]
    async fn deletion_requires_success_and_rejects_unsafe_tokens() {
        let (endpoint, request, task) = server("200 OK", r#"{"success":true,"data":true}"#);
        let mut api = Imgur::new("test-client").unwrap();
        api.endpoint = endpoint;
        assert!(api.delete("../other").await.is_err());
        api.delete("secret123").await.unwrap();
        assert!(
            String::from_utf8_lossy(&request.recv().unwrap())
                .starts_with("DELETE /3/image/secret123 HTTP/1.1")
        );
        task.join().unwrap();
    }
    #[tokio::test]
    async fn http_and_api_failures_are_not_success() {
        for (status, body) in [
            ("429 Too Many Requests", r#"{"success":false}"#),
            ("200 OK", r#"{"success":false}"#),
            ("200 OK", "invalid json"),
        ] {
            let (endpoint, request, task) = server(status, body);
            let mut api = Imgur::new("test").unwrap();
            api.endpoint = endpoint;
            assert!(api.delete("secret").await.is_err());
            request.recv().unwrap();
            task.join().unwrap();
        }
    }
}
