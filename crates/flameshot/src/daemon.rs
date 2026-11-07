// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::{Context, Result};
use flameshot_core::qt_wire;
use futures_util::StreamExt;
use std::{
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use zbus::{Connection, Proxy};
#[derive(Debug)]
pub enum Stop {
    Interrupt,
    Terminate,
    Disconnected,
    ConnectionFailed,
}
impl Stop {
    pub fn code(&self) -> u8 {
        match self {
            Self::Interrupt => 130,
            Self::Terminate => 143,
            Self::Disconnected | Self::ConnectionFailed => 3,
        }
    }
}
impl std::fmt::Display for Stop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Interrupt => "Interrupted",
            Self::Terminate => "Terminated",
            Self::Disconnected => "Session D-Bus disconnected",
            Self::ConnectionFailed => "Cannot connect to session D-Bus",
        })
    }
}
impl std::error::Error for Stop {}
pub async fn session_connection() -> Result<Connection> {
    Connection::session().await.context(Stop::ConnectionFailed)
}
const SERVICE: &str = "org.flameshot.Flameshot";
#[derive(Default)]
pub(crate) struct Activity {
    clipboard: Mutex<Vec<flameshot_platform::clipboard::Lease>>,
    children: Arc<crate::children::Children>,
    used: AtomicBool,
}
impl Activity {
    pub(crate) fn launch(self: &Arc<Self>, command: &str) -> Result<()> {
        self.launch_args(&[command])
    }
    pub(crate) fn launch_args(self: &Arc<Self>, args: &[&str]) -> Result<()> {
        self.children.spawn(
            crate::children::owned_command()?
                .args(args)
                .stdout(Stdio::null()),
            None,
        )?;
        self.used.store(true, Ordering::Release);
        Ok(())
    }
    fn idle(&self) -> bool {
        self.used.load(Ordering::Acquire)
            && self.children.idle()
            && self.clipboard.try_lock().is_ok_and(|mut leases| {
                leases.retain(|lease| lease.active());
                leases.is_empty()
            })
    }
}
struct Service {
    activity: Arc<Activity>,
}
fn failure(error: impl std::fmt::Display) -> zbus::fdo::Error {
    zbus::fdo::Error::Failed(error.to_string())
}
impl Service {
    fn clipboard(&self, data: Vec<u8>, mime: &str) -> zbus::fdo::Result<()> {
        let mut lease = self.activity.clipboard.lock().map_err(failure)?;
        let wayland = std::env::var("XDG_SESSION_TYPE").as_deref() == Ok("wayland")
            || std::env::var_os("WAYLAND_DISPLAY").is_some();
        let provider = flameshot_platform::clipboard::copy(data, mime, wayland).map_err(failure)?;
        lease.retain(|provider| provider.active());
        lease.push(provider);
        self.activity.used.store(true, Ordering::Release);
        Ok(())
    }
}
#[zbus::interface(name = "org.flameshot.Flameshot")]
impl Service {
    #[zbus(name = "captureScreen")]
    fn capture_screen(&self) -> zbus::fdo::Result<()> {
        self.activity.launch("gui").map_err(failure)
    }
    #[zbus(name = "attachScreenshotToClipboard")]
    fn attach_screenshot_to_clipboard(&self, data: Vec<u8>) -> zbus::fdo::Result<()> {
        let image = qt_wire::decode_clipboard(&data)
            .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;
        let jpeg = crate::settings::Settings::load()
            .map_err(failure)?
            .jpeg_clipboard;
        let data = crate::output::encode(
            &image,
            if jpeg {
                image::ImageFormat::Jpeg
            } else {
                image::ImageFormat::Png
            },
        )
        .map_err(failure)?;
        self.clipboard(data, if jpeg { "image/jpeg" } else { "image/png" })
    }
    #[zbus(name = "attachTextToClipboard")]
    async fn attach_text_to_clipboard(
        &self,
        text: &str,
        notification: &str,
        #[zbus(connection)] connection: &Connection,
    ) -> zbus::fdo::Result<()> {
        self.clipboard(text.as_bytes().to_vec(), "text/plain;charset=utf-8")?;
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
        self.activity
            .children
            .spawn(
                crate::children::owned_command()
                    .map_err(failure)?
                    .arg("internal-pin")
                    .stdout(Stdio::null()),
                Some(data),
            )
            .map_err(failure)?;
        self.activity.used.store(true, Ordering::Release);
        Ok(())
    }
}
pub async fn run() -> Result<()> {
    // Install handlers before publishing the service name as readiness.
    let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let activity = Arc::new(Activity::default());
    let _children_shutdown = activity.children.guard();
    let connection = zbus::connection::Builder::session()
        .context(Stop::ConnectionFailed)?
        .serve_at(
            "/",
            Service {
                activity: activity.clone(),
            },
        )?
        .build()
        .await
        .context(Stop::ConnectionFailed)?;
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
    #[cfg(feature = "update-check")]
    {
        let connection = connection.clone();
        tokio::spawn(async move {
            loop {
                if let Ok(settings) = crate::settings::Settings::load()
                    && settings.check_updates
                {
                    match flameshot_platform::updates::check(
                        env!("CARGO_PKG_VERSION"),
                        &settings.ignored_update,
                    )
                    .await
                    {
                        Ok(Some(update)) if settings.notifications => {
                            let _ = flameshot_platform::desktop::notify(
                                &connection,
                                "Flameshot",
                                &format!("Version {} is available: {}", update.version, update.url),
                            )
                            .await;
                        }
                        Err(error) => eprintln!("flameshot: Update check failed: {error}"),
                        _ => {}
                    }
                }
                tokio::time::sleep(Duration::from_secs(24 * 60 * 60)).await;
            }
        });
    }
    use ksni::TrayMethods;
    let quit = std::sync::Arc::new(tokio::sync::Notify::new());
    let mut tray_handle = None;
    let mut refresh = tokio::time::interval(Duration::from_secs(1));
    let mut messages = zbus::MessageStream::from(&connection);

    loop {
        tokio::select! {
            _=quit.notified()=>break,
            _=refresh.tick()=>{
                if let Ok(settings)=crate::settings::Settings::load() {
                    if settings.auto_close_idle && activity.idle() {break;}
                    if settings.disabled_tray {if let Some(handle)=tray_handle.take() {let handle:ksni::Handle<crate::tray::Tray>=handle;handle.shutdown().await;}}
                    else if tray_handle.is_none() {
                        tray_handle=crate::tray::Tray::new(quit.clone(),activity.clone()).disable_dbus_name(crate::app_ui::restricted_bus_names()).spawn().await.ok();
                        if tray_handle.is_some() && settings.startup_message {
                            let connection=connection.clone();
                            tokio::spawn(async move {
                                let _=flameshot_platform::desktop::notify(&connection,"Flameshot",crate::i18n::tr("QObject","Hello, I'm here! Click icon in the tray to take a screenshot or click with a right button to see more options.")).await;
                            });
                        }
                    }
                }
            },
            _=interrupt.recv()=>return Err(Stop::Interrupt.into()),
            _=terminate.recv()=>return Err(Stop::Terminate.into()),
            message=messages.next()=>{if message.is_none() || message.is_some_and(|m|m.is_err()) {return Err(Stop::Disconnected.into());}}
        }
    }
    Ok(())
}
async fn connect_service() -> Result<Connection> {
    let connection = session_connection().await?;
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

pub async fn copy_text(text: &str) -> Result<()> {
    let connection = connect_service().await?;
    let proxy = Proxy::new(&connection, SERVICE, "/", SERVICE).await?;
    proxy
        .call::<_, _, ()>("attachTextToClipboard", &(text, ""))
        .await?;
    Ok(())
}
