// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::{Context, Result};
use flameshot_core::qt_wire;
use futures_util::StreamExt;
use std::{
    borrow::Cow,
    io::Write,
    process::{Command, Stdio},
    sync::Mutex,
    time::Duration,
};
use zbus::{Connection, Proxy};
const SERVICE: &str = "org.flameshot.Flameshot";
#[derive(Default)]
struct Service {
    clipboard: Mutex<Option<arboard::Clipboard>>,
}
fn failure(error: impl std::fmt::Display) -> zbus::fdo::Error {
    zbus::fdo::Error::Failed(error.to_string())
}
impl Service {
    fn clipboard(
        &self,
        action: impl FnOnce(&mut arboard::Clipboard) -> Result<(), arboard::Error>,
    ) -> zbus::fdo::Result<()> {
        let mut guard = self.clipboard.lock().map_err(failure)?;
        if guard.is_none() {
            *guard = Some(arboard::Clipboard::new().map_err(failure)?);
        }
        action(guard.as_mut().unwrap()).map_err(failure)
    }
}
#[zbus::interface(name = "org.flameshot.Flameshot")]
impl Service {
    #[zbus(name = "captureScreen")]
    fn capture_screen(&self) -> zbus::fdo::Result<()> {
        let mut child = Command::new(std::env::current_exe().map_err(failure)?)
            .arg("gui")
            .stdout(Stdio::null())
            .spawn()
            .map_err(failure)?;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }
    #[zbus(name = "attachScreenshotToClipboard")]
    fn attach_screenshot_to_clipboard(&self, data: Vec<u8>) -> zbus::fdo::Result<()> {
        let image = qt_wire::decode_clipboard(&data)
            .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;
        self.clipboard(|clipboard| {
            clipboard.set_image(arboard::ImageData {
                width: image.width() as usize,
                height: image.height() as usize,
                bytes: Cow::Owned(image.into_raw()),
            })
        })
    }
    #[zbus(name = "attachTextToClipboard")]
    async fn attach_text_to_clipboard(
        &self,
        text: &str,
        notification: &str,
        #[zbus(connection)] connection: &Connection,
    ) -> zbus::fdo::Result<()> {
        self.clipboard(|clipboard| clipboard.set_text(text))?;
        if !notification.is_empty()
            && crate::settings::Settings::load()
                .map(|settings| settings.notifications)
                .unwrap_or(true)
        {
            let _ = tokio::time::timeout(Duration::from_secs(2), async {
                let proxy = Proxy::new(
                    connection,
                    "org.freedesktop.Notifications",
                    "/org/freedesktop/Notifications",
                    "org.freedesktop.Notifications",
                )
                .await?;
                proxy
                    .call::<_, _, u32>(
                        "Notify",
                        &(
                            "Flameshot",
                            0_u32,
                            "org.flameshot.Flameshot",
                            "Flameshot",
                            notification,
                            Vec::<String>::new(),
                            std::collections::HashMap::<String, zbus::zvariant::Value>::new(),
                            -1_i32,
                        ),
                    )
                    .await
            })
            .await;
        }
        Ok(())
    }
    #[zbus(name = "attachPin")]
    fn attach_pin(&self, data: Vec<u8>) -> zbus::fdo::Result<()> {
        qt_wire::decode_pin(&data).map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;
        let mut child = Command::new(std::env::current_exe().map_err(failure)?)
            .arg("internal-pin")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .map_err(failure)?;
        let mut input = child
            .stdin
            .take()
            .ok_or_else(|| failure("Missing pin input pipe"))?;
        std::thread::spawn(move || {
            let _ = input.write_all(&data);
            drop(input);
            let _ = child.wait();
        });
        Ok(())
    }
}
pub async fn run() -> Result<()> {
    let connection = zbus::connection::Builder::session()?
        .serve_at("/", Service::default())?
        .build()
        .await?;
    let reply = match connection
        .request_name_with_flags(SERVICE, zbus::fdo::RequestNameFlags::DoNotQueue.into())
        .await
    {
        Ok(reply) => reply,
        Err(zbus::Error::NameTaken) => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if reply != zbus::fdo::RequestNameReply::PrimaryOwner {
        return Ok(());
    }
    use ksni::TrayMethods;
    let quit = std::sync::Arc::new(tokio::sync::Notify::new());
    let mut tray_handle = None;
    let mut refresh = tokio::time::interval(Duration::from_secs(1));
    let mut messages = zbus::MessageStream::from(&connection);
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    loop {
        tokio::select! {
            _=quit.notified()=>break,
            _=refresh.tick()=>{
                if let Ok(settings)=crate::settings::Settings::load() {
                    if settings.disabled_tray {if let Some(handle)=tray_handle.take() {let handle:ksni::Handle<crate::tray::Tray>=handle;handle.shutdown().await;}}
                    else if tray_handle.is_none() {tray_handle=crate::tray::Tray{quit:quit.clone()}.spawn().await.ok();}
                }
            },
            _=tokio::signal::ctrl_c()=>break,
            _=terminate.recv()=>break,
            message=messages.next()=>{if message.is_none() || message.is_some_and(|m|m.is_err()) {break;}}
        }
    }
    Ok(())
}
async fn connect_service() -> Result<Connection> {
    let connection = Connection::session().await?;
    let bus = zbus::fdo::DBusProxy::new(&connection).await?;
    let name = zbus::names::BusName::try_from(SERVICE)?;
    if !bus.name_has_owner(name.clone()).await? {
        let mut child = Command::new(std::env::current_exe()?)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if bus.name_has_owner(name.clone()).await? {
                    return Ok::<_, zbus::Error>(());
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .context("Timed out starting clipboard daemon")??;
    }
    Ok(connection)
}
pub async fn copy(image: &image::RgbaImage) -> Result<()> {
    let connection = connect_service().await?;
    let proxy = Proxy::new(&connection, SERVICE, "/", SERVICE).await?;
    proxy
        .call::<_, _, ()>(
            "attachScreenshotToClipboard",
            &(qt_wire::encode_clipboard(image)?,),
        )
        .await?;
    Ok(())
}

pub async fn pin(
    image: &image::RgbaImage,
    selection: flameshot_core::geometry::PixelRect,
) -> Result<()> {
    let connection = connect_service().await?;
    let proxy = Proxy::new(&connection, SERVICE, "/", SERVICE).await?;
    let pin = qt_wire::Pin {
        image: image.clone(),
        geometry: qt_wire::PinGeometry {
            x: selection.x.try_into()?,
            y: selection.y.try_into()?,
            width: selection.width,
            height: selection.height,
        },
        scale: 1.0,
    };
    proxy
        .call::<_, _, ()>("attachPin", &(qt_wire::encode_pin(&pin)?,))
        .await?;
    Ok(())
}
