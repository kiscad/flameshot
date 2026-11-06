// SPDX-License-Identifier: GPL-3.0-or-later
mod config_ui;
mod daemon;
mod gui;
mod i18n;
mod output;
mod pin;
mod settings;
mod tray;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use flameshot_platform::{portal, screens, x11};
use std::{
    io::{self, Write},
    path::PathBuf,
    time::Duration,
};

#[derive(Parser)]
#[command(
    name = "flameshot",
    version,
    about = "Flameshot Rust migration — desktop screenshot utility"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}
#[derive(Subcommand)]
enum Command {
    /// Open configuration, or change individual settings.
    Config {
        #[arg(short = 'a', long)]
        autostart: Option<bool>,
        #[arg(short = 'f', long)]
        filename: Option<String>,
        #[arg(short = 't', long)]
        trayicon: Option<bool>,
        #[arg(short = 'n', long)]
        notifications: Option<bool>,
        #[arg(short = 's', long)]
        showhelp: Option<bool>,
    },
    #[command(hide = true)]
    InternalPin,
    /// Capture a monitor (defaults to the monitor containing the pointer on X11).
    Screen {
        #[arg(long)]
        pin: bool,
        #[arg(short = 'n', long)]
        number: Option<usize>,
        #[arg(short = 'e', long)]
        edit: bool,
        #[arg(short = 'p', long)]
        path: Option<PathBuf>,
        #[arg(short = 'd', long, default_value_t = 0)]
        delay: u64,
        #[arg(short = 'c', long)]
        clipboard: bool,
        #[arg(short = 'r', long)]
        raw: bool,
    },
    /// Select and annotate a screenshot.
    Gui {
        #[arg(short = 's', long)]
        accept_on_select: bool,
        #[arg(short = 'g', long)]
        print_geometry: bool,
        #[arg(long)]
        pin: bool,
        #[arg(short = 'p', long)]
        path: Option<PathBuf>,
        #[arg(short = 'd', long, default_value_t = 0)]
        delay: u64,
        #[arg(short = 'c', long)]
        clipboard: bool,
        #[arg(short = 'r', long)]
        raw: bool,
    },
    /// Capture the entire desktop.
    Full {
        #[arg(short = 'p', long)]
        path: Option<PathBuf>,
        #[arg(short = 'd', long, default_value_t = 0)]
        delay: u64,
        #[arg(short = 'c', long)]
        clipboard: bool,
        #[arg(short = 'r', long)]
        raw: bool,
    },
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    match run(cli).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("flameshot: {error:#}");
            let code = if matches!(
                error.downcast_ref::<portal::CaptureError>(),
                Some(portal::CaptureError::Cancelled)
            ) {
                2
            } else {
                1
            };
            std::process::ExitCode::from(code)
        }
    }
}

async fn run(cli: Cli) -> Result<()> {
    let Some(command) = cli.command else {
        return daemon::run().await;
    };
    if let Command::Config {
        autostart,
        filename,
        trayicon,
        notifications,
        showhelp,
    } = &command
    {
        let mut values = Vec::new();
        if let Some(value) = filename {
            values.push(("filenamePattern", value.clone()));
        }
        if let Some(value) = trayicon {
            values.push(("disabledTrayIcon", (!value).to_string()));
        }
        if let Some(value) = notifications {
            values.push(("showDesktopNotification", value.to_string()));
        }
        if let Some(value) = showhelp {
            values.push(("showHelp", value.to_string()));
        }
        if !values.is_empty() {
            settings::Settings::update(&values)?;
        }
        if let Some(value) = autostart {
            settings::Settings::set_startup(*value)?;
        }
        return if values.is_empty() && autostart.is_none() {
            config_ui::show()
        } else {
            Ok(())
        };
    }
    if matches!(command, Command::InternalPin) {
        use std::io::Read;
        let mut data = Vec::new();
        std::io::stdin()
            .take(128 * 1024 * 1024 + 1)
            .read_to_end(&mut data)?;
        return pin::show(flameshot_core::qt_wire::decode_pin(&data)?);
    }
    let settings = settings::Settings::load()?;
    let accept_on_select = matches!(
        &command,
        Command::Gui {
            accept_on_select: true,
            ..
        }
    );
    let print_geometry = matches!(
        &command,
        Command::Gui {
            print_geometry: true,
            ..
        }
    );
    let requested_pin = matches!(
        &command,
        Command::Gui { pin: true, .. } | Command::Screen { pin: true, .. }
    );
    let graphical = matches!(
        command,
        Command::Gui { .. } | Command::Screen { edit: true, .. }
    );
    let screen_request = if let Command::Screen { number, .. } = &command {
        Some(*number)
    } else {
        None
    };
    let selected_screen = if let Some(number) = screen_request {
        let wayland = is_wayland();
        let monitors = tokio::task::spawn_blocking(move || screens::list(wayland)).await??;
        let index = if let Some(index) = number {
            index
        } else if wayland {
            anyhow::bail!("Wayland does not expose the global pointer position; specify --number");
        } else {
            let (x, y) = x11::pointer_position()?;
            screens::at_point(&monitors, f64::from(x), f64::from(y))
                .context("Pointer is outside active monitors")?
        };
        anyhow::ensure!(index < monitors.len(), "Screen number is out of range");
        Some((monitors, index))
    } else {
        None
    };
    match command {
        Command::InternalPin | Command::Config { .. } => unreachable!(),
        Command::Full {
            path,
            delay,
            raw,
            clipboard,
        }
        | Command::Gui {
            path,
            delay,
            raw,
            clipboard,
            ..
        }
        | Command::Screen {
            path,
            delay,
            raw,
            clipboard,
            ..
        } => {
            // Reject invalid destinations before taking a potentially sensitive screenshot.
            if let Some(path) = &path {
                output::validate_destination(path)?;
            }
            tokio::time::sleep(Duration::from_millis(delay)).await;
            let connection = zbus::Connection::session()
                .await
                .context("Cannot connect to session D-Bus")?;
            let capture = match portal::screenshot(&connection, "", Duration::from_secs(15)).await {
                Ok(path) => {
                    let reader = image::ImageReader::open(path)?.with_guessed_format()?;
                    reader
                        .decode()
                        .context("Cannot decode portal screenshot")?
                        .to_rgba8()
                }
                Err(error) if !is_wayland() && unavailable(&error) => {
                    eprintln!("flameshot: Screenshot portal unavailable, using direct X11 capture");
                    tokio::task::spawn_blocking(x11::capture_desktop).await??
                }
                Err(error) => return Err(error.into()),
            };
            let capture = if let Some((monitors, index)) = selected_screen {
                let bounds =
                    screens::crop_bounds(&monitors, index, capture.width(), capture.height())?;
                image::imageops::crop_imm(&capture, bounds.x, bounds.y, bounds.width, bounds.height)
                    .to_image()
            } else {
                capture
            };
            let outcome = if graphical {
                gui::edit(
                    capture,
                    gui::Options {
                        accept_on_select,
                        save_on_accept: path.is_some()
                            || (!raw && !clipboard && !requested_pin && !print_geometry),
                        save_path: path.clone(),
                    },
                )?
                .ok_or(portal::CaptureError::Cancelled)?
            } else {
                let selection = flameshot_core::geometry::PixelRect {
                    x: 0,
                    y: 0,
                    width: capture.width(),
                    height: capture.height(),
                };
                gui::Outcome {
                    image: capture,
                    selection,
                    action: gui::Action::Accept,
                    saved: false,
                }
            };
            let capture = outcome.image;
            let copy = clipboard || outcome.action == gui::Action::Copy;
            let pin = requested_pin || outcome.action == gui::Action::Pin;
            if copy {
                daemon::copy(&capture).await?;
            }
            if pin {
                daemon::pin(&capture, outcome.selection).await?;
            }
            let mut saved = outcome.saved;
            if !outcome.saved
                && (path.is_some()
                    || outcome.action == gui::Action::Save
                    || (!raw && !copy && !pin && !print_geometry))
            {
                let path = path.unwrap_or_else(|| {
                    settings
                        .save_path
                        .clone()
                        .or_else(dirs::picture_dir)
                        .or_else(dirs::home_dir)
                        .unwrap_or_else(|| PathBuf::from("."))
                });
                let saved_path = output::save_with_settings(&capture, &path, &settings)?;
                saved = true;
                eprintln!("flameshot: Capture saved as {}", saved_path.display());
            }
            if settings.notifications && (copy || pin || saved) {
                let body = if copy {
                    "Capture copied to clipboard"
                } else if pin {
                    "Capture pinned"
                } else {
                    "Capture saved"
                };
                let _ = flameshot_platform::desktop::notify(&connection, "Flameshot", body).await;
            }
            if raw {
                let png = output::encode(&capture, image::ImageFormat::Png)?;
                io::stdout().lock().write_all(&png)?;
            } else if print_geometry {
                let region = outcome.selection;
                writeln!(
                    io::stdout().lock(),
                    "{}x{}+{}+{}",
                    region.width,
                    region.height,
                    region.x,
                    region.y
                )?;
            }
        }
    }
    Ok(())
}
fn is_wayland() -> bool {
    std::env::var("XDG_SESSION_TYPE").as_deref() == Ok("wayland")
        || std::env::var_os("WAYLAND_DISPLAY").is_some()
}
fn unavailable(error: &portal::CaptureError) -> bool {
    match error {
        portal::CaptureError::Bus(zbus::Error::MethodError(name, _, _)) => matches!(
            name.as_str(),
            "org.freedesktop.DBus.Error.ServiceUnknown"
                | "org.freedesktop.DBus.Error.UnknownMethod"
                | "org.freedesktop.DBus.Error.UnknownInterface"
        ),
        _ => false,
    }
}
