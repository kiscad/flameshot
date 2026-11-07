mod about;
mod app_ui;
mod buttons;
mod children;
mod drawing_grid;
mod geometry_overlay;
mod size_keyboard;
mod text_edit;
mod theme;
#[cfg(all(test, any(feature = "imgur", feature = "update-check")))]
mod ui_test;
mod wheel;
// SPDX-License-Identifier: GPL-3.0-or-later
mod config_check;
mod config_ui;
mod daemon;
mod gui;
mod i18n;
mod jobs;
mod launcher;
#[cfg(any(feature = "imgur", feature = "update-check"))]
mod links;
mod open_with;
mod output;
mod pin;
mod region_cache;
mod settings;
mod shortcuts;
mod tray;
#[cfg(feature = "update-check")]
mod updates_ui;
#[cfg(feature = "imgur")]
mod upload_history;
#[cfg(feature = "imgur")]
mod upload_ui;
use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand, builder::TypedValueParser};
use flameshot_platform::{portal, screens, x11};
use std::{
    io::{self, Write},
    path::PathBuf,
    time::Duration,
};

// The legacy CLI numeric checker used QString::toInt and required value >= 0.
fn parse_cli_nonnegative(value: &str) -> Result<u64, String> {
    value
        .trim()
        .parse::<i32>()
        .ok()
        .filter(|value| *value >= 0)
        .map(|value| value as u64)
        .ok_or_else(|| "Expected an integer between 0 and 2147483647".into())
}
fn parse_cli_screen_number(value: &str) -> Result<usize, String> {
    parse_cli_nonnegative(value).map(|value| value as usize)
}

fn parse_cli_color(value: &str) -> Result<String, String> {
    let color = if value.starts_with('#') {
        flameshot_core::color::parse_palette(value)
    } else {
        let name: String = value.chars().filter(|ch| !ch.is_whitespace()).collect();
        if name.starts_with('#') {
            None
        } else {
            flameshot_core::color::parse(&name)
        }
    };
    color
        .filter(|rgba| rgba[3] == 255)
        .map(flameshot_core::color::encode_palette)
        .ok_or_else(|| "Expected an opaque QColor name or hexadecimal color".into())
}
fn cli_path_parser() -> impl TypedValueParser<Value = PathBuf> {
    clap::builder::PathBufValueParser::new().try_map(|path: PathBuf| {
        let parent = path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."));
        if path.is_dir() || parent.is_dir() {
            Ok(path)
        } else {
            Err("Expected an existing directory or a file in an existing directory".to_owned())
        }
    })
}

#[derive(Parser)]
#[command(
    name = "flameshot",
    version,
    disable_version_flag = true,
    about = "Powerful yet simple-to-use screenshot software"
)]
struct Cli {
    #[arg(long, hide = true)]
    internal_parent: Option<u32>,
    #[arg(short = 'v', long = "version", visible_short_alias = 'V', action = clap::ArgAction::Version, help = "Display version information")]
    _version: Option<bool>,
    #[command(subcommand)]
    command: Option<Command>,
}
#[derive(Subcommand)]
enum Command {
    #[cfg(feature = "update-check")]
    Updates,
    #[cfg(feature = "imgur")]
    History,
    /// Open the capture launcher.
    Launcher,
    /// Open configuration, or change individual settings.
    Config {
        #[arg(short = 'm', long, action = clap::ArgAction::Append, value_parser = parse_cli_color)]
        maincolor: Option<String>,
        #[arg(short = 'k', long, action = clap::ArgAction::Append, value_parser = parse_cli_color)]
        contrastcolor: Option<String>,
        #[arg(long, overrides_with = "check")]
        check: bool,
        #[arg(short = 'a', long, action = clap::ArgAction::Append)]
        autostart: Option<bool>,
        #[arg(short = 'f', long, action = clap::ArgAction::Append)]
        filename: Option<String>,
        #[arg(short = 't', long, action = clap::ArgAction::Append)]
        trayicon: Option<bool>,
        #[arg(short = 'n', long, action = clap::ArgAction::Append)]
        notifications: Option<bool>,
        #[arg(short = 's', long, action = clap::ArgAction::Append)]
        showhelp: Option<bool>,
    },
    #[command(hide = true)]
    InternalPin,
    #[command(hide = true)]
    InternalAbout,
    #[command(hide = true)]
    InternalOpenSavePath,
    /// Capture a monitor (defaults to the monitor containing the pointer on X11).
    Screen {
        #[arg(long, action = clap::ArgAction::Append)]
        region: Option<flameshot_core::region::Spec>,
        #[arg(long, overrides_with = "pin")]
        pin: bool,
        #[arg(short = 'n', long, value_parser = parse_cli_screen_number, action = clap::ArgAction::Append)]
        number: Option<usize>,
        #[arg(short = 'e', long, overrides_with = "edit")]
        edit: bool,
        #[arg(short = 'p', long, action = clap::ArgAction::Append, value_parser = cli_path_parser())]
        path: Option<PathBuf>,
        #[arg(short = 'd', long, default_value_t = 0, value_parser = parse_cli_nonnegative, action = clap::ArgAction::Append)]
        delay: u64,
        #[arg(short = 'c', long, overrides_with = "clipboard")]
        clipboard: bool,
        #[arg(short = 'r', long, overrides_with = "raw")]
        raw: bool,
    },
    /// Select and annotate a screenshot.
    Gui {
        #[arg(long, action = clap::ArgAction::Append)]
        region: Option<flameshot_core::region::Spec>,
        #[arg(long, overrides_with = "last_region")]
        last_region: bool,
        #[arg(short = 's', long, overrides_with = "accept_on_select")]
        accept_on_select: bool,
        #[arg(short = 'g', long, overrides_with = "print_geometry")]
        print_geometry: bool,
        #[arg(long, overrides_with = "pin")]
        pin: bool,
        #[arg(short = 'p', long, action = clap::ArgAction::Append, value_parser = cli_path_parser())]
        path: Option<PathBuf>,
        #[arg(short = 'd', long, default_value_t = 0, value_parser = parse_cli_nonnegative, action = clap::ArgAction::Append)]
        delay: u64,
        #[arg(short = 'c', long, overrides_with = "clipboard")]
        clipboard: bool,
        #[arg(short = 'r', long, overrides_with = "raw")]
        raw: bool,
    },
    /// Capture the entire desktop.
    Full {
        #[arg(short = 'p', long, action = clap::ArgAction::Append, value_parser = cli_path_parser())]
        path: Option<PathBuf>,
        #[arg(short = 'd', long, default_value_t = 0, value_parser = parse_cli_nonnegative, action = clap::ArgAction::Append)]
        delay: u64,
        #[arg(short = 'c', long, overrides_with = "clipboard")]
        clipboard: bool,
        #[arg(short = 'r', long, overrides_with = "raw")]
        raw: bool,
    },
}

fn parse_cli_from<I, T>(arguments: I) -> Result<Cli, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString>,
{
    use clap::error::ErrorKind;
    let mut args: Vec<std::ffi::OsString> = arguments.into_iter().map(Into::into).collect();
    // The documented Rust help subcommand is an extension to the legacy CLI.
    if args.get(1).is_some_and(|arg| arg == "help") {
        return Cli::try_parse_from(args);
    }
    let mut schema = Cli::command();
    schema.build();
    let mut node = &schema;
    let mut help_position = true;
    let mut index = 1;
    let shape_error = |args: &[std::ffi::OsString], index, message: String| {
        // A malformed earlier value wins over a later syntax/help error.
        Cli::try_parse_from(&args[..index])
            .err()
            .unwrap_or_else(|| clap::Error::raw(ErrorKind::InvalidValue, message))
    };
    while index < args.len() {
        let text = args[index].to_string_lossy().into_owned();
        if !text.starts_with('-') {
            let Some(command) = node.find_subcommand(&text) else {
                return Err(shape_error(
                    &args,
                    index,
                    format!("'{text}' is not a valid argument"),
                ));
            };
            node = command;
            help_position = true;
            index += 1;
            continue;
        }
        let (name, inline) = text
            .split_once('=')
            .map_or((text.as_str(), None), |(name, value)| (name, Some(value)));
        let long = name.starts_with("--");
        if (long && name.len() <= 3) || (!long && name.chars().count() != 2) {
            return Err(shape_error(
                &args,
                index,
                format!("The option '{name}' has a wrong format"),
            ));
        }
        let argument = node.get_arguments().find(|argument| {
            if long {
                argument.get_long() == Some(&name[2..])
                    || argument
                        .get_all_aliases()
                        .is_some_and(|aliases| aliases.contains(&&name[2..]))
            } else {
                let short = name.chars().nth(1).unwrap();
                argument.get_short() == Some(short)
                    || argument
                        .get_all_short_aliases()
                        .is_some_and(|aliases| aliases.contains(&short))
            }
        });
        let Some(argument) = argument else {
            return Err(shape_error(
                &args,
                index,
                format!("The option '{name}' is not valid for '{}'", node.get_name()),
            ));
        };
        match argument.get_action() {
            clap::ArgAction::Help | clap::ArgAction::HelpShort | clap::ArgAction::HelpLong => {
                if !help_position || inline.is_some() || index + 1 != args.len() {
                    return Err(shape_error(&args, index, "Help must appear immediately after the command and have no following arguments".into()));
                }
                Cli::try_parse_from(&args[..index])?;
                return Cli::try_parse_from(args);
            }
            clap::ArgAction::Version => {
                if index != 1 || inline.is_some() || args.len() != 2 {
                    return Err(shape_error(
                        &args,
                        index,
                        "Invalid arguments after or before the version option".into(),
                    ));
                }
                return Cli::try_parse_from(args);
            }
            action if action.takes_values() => {
                if inline.is_none_or(str::is_empty) {
                    if index + 1 == args.len() {
                        return Err(shape_error(
                            &args,
                            index,
                            format!("Expected value after the option '{name}'"),
                        ));
                    }
                    // Legacy options consume the next argument verbatim, even
                    // when it starts with '-'. Preserve native path bytes.
                    let mut combined = std::ffi::OsString::from(name);
                    combined.push("=");
                    combined.push(args.remove(index + 1));
                    args[index] = combined;
                }
            }
            _ if inline.is_some() => {
                return Err(shape_error(
                    &args,
                    index,
                    format!("The option '{name}' does not take a value"),
                ));
            }
            _ => {}
        }
        help_position = false;
        index += 1;
    }
    Cli::try_parse_from(args)
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let cli = match parse_cli_from(std::env::args_os()) {
        Ok(cli) => cli,
        Err(error) => {
            let _ = error.print();
            // The legacy main returns zero through `finish` after parse failure.
            return std::process::ExitCode::SUCCESS;
        }
    };
    if let Some(parent) = cli.internal_parent {
        match children::bind_owner(parent) {
            Ok(true) => {}
            Ok(false) => return std::process::ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("flameshot: {error:#}");
                return std::process::ExitCode::FAILURE;
            }
        }
    }
    match run(cli).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            if let Some(stop) = error.downcast_ref::<daemon::Stop>() {
                if matches!(
                    stop,
                    daemon::Stop::Disconnected | daemon::Stop::ConnectionFailed
                ) {
                    eprintln!("flameshot: {error:#}");
                }
                return std::process::ExitCode::from(stop.code());
            }
            eprintln!("flameshot: {error:#}");
            let code = if matches!(
                error.downcast_ref::<portal::CaptureError>(),
                Some(portal::CaptureError::Cancelled)
            ) {
                if settings::Settings::load()
                    .is_ok_and(|settings| settings.notifications && settings.abort_notifications)
                {
                    let _ = tokio::time::timeout(Duration::from_secs(2), async {
                        let connection = zbus::Connection::session().await?;
                        flameshot_platform::desktop::notify(
                            &connection,
                            "Flameshot",
                            "Screenshot aborted.",
                        )
                        .await
                    })
                    .await;
                }
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
    #[cfg(feature = "imgur")]
    if matches!(command, Command::History) {
        return upload_ui::history();
    }
    #[cfg(feature = "update-check")]
    if matches!(command, Command::Updates) {
        return updates_ui::show();
    }
    if matches!(command, Command::InternalOpenSavePath) {
        if let Some(directory) = settings::Settings::load()?.save_path {
            if app_ui::restricted_bus_names() {
                let connection = daemon::session_connection().await?;
                flameshot_platform::open_portal::open_directory(
                    &connection,
                    &directory,
                    Duration::from_secs(120),
                )
                .await?;
            } else {
                flameshot_platform::applications::open_directory(&directory)?;
            }
        }
        return Ok(());
    }
    if matches!(command, Command::InternalAbout) {
        return about::show();
    }
    if matches!(command, Command::Launcher) {
        return launcher::show();
    }
    if let Command::Config {
        maincolor,
        contrastcolor,
        check,
        autostart,
        filename,
        trayicon,
        notifications,
        showhelp,
    } = &command
    {
        if *check {
            let path = dirs::config_dir()
                .context("Cannot locate configuration directory")?
                .join("flameshot/flameshot.ini");
            let source = match std::fs::read_to_string(path) {
                Ok(source) => source,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
                Err(error) => return Err(error.into()),
            };
            let errors = config_check::check(&source);
            anyhow::ensure!(errors.is_empty(), "{}", errors.join("\n"));
            println!("No errors detected.");
            return Ok(());
        }
        let mut values = Vec::new();
        if let Some(value) = maincolor {
            values.push(("uiColor", value.clone()));
        }
        if let Some(value) = contrastcolor {
            values.push(("contrastUiColor", value.clone()));
        }
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
    let _capture_lock = if graphical && !settings.allow_multiple_gui {
        let directory = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .or_else(dirs::cache_dir)
            .context("No capture-lock directory available")?;
        Some(
            flameshot_platform::capture_lock::acquire(&directory.join("flameshot/gui.lock"))?
                .context("A screenshot editor is already running")?,
        )
    } else {
        None
    };
    use flameshot_core::region::{Region, Spec};
    let region_spec = match &command {
        Command::Gui { region, .. } | Command::Screen { region, .. } => *region,
        _ => None,
    };
    let last_region = matches!(
        &command,
        Command::Gui {
            last_region: true,
            ..
        }
    ) || (matches!(&command, Command::Gui { .. }) && settings.use_last_region);
    anyhow::ensure!(
        !(matches!(&command, Command::Screen { .. })
            && matches!(region_spec, Some(Spec::Screen(_)))),
        "The screen command does not support --region screen<N>"
    );
    let screen_request = if let Command::Screen { number, .. } = &command {
        Some(*number)
    } else if let Some(Spec::Screen(index)) = region_spec {
        Some(Some(index))
    } else {
        None
    };
    let mut selected_screen = if let Some(number) = screen_request {
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
    let mut available_screens = if graphical && selected_screen.is_none() {
        let wayland = is_wayland();
        Some(tokio::task::spawn_blocking(move || screens::list(wayland)).await??)
    } else {
        None
    };
    match command {
        #[cfg(feature = "update-check")]
        Command::Updates => unreachable!(),
        #[cfg(feature = "imgur")]
        Command::History => unreachable!(),
        Command::InternalPin
        | Command::InternalOpenSavePath
        | Command::InternalAbout
        | Command::Config { .. }
        | Command::Launcher => unreachable!(),
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
            if settings.capture_active_monitor
                && let Some(monitors) = &available_screens
                && monitors.len() > 1
            {
                anyhow::ensure!(
                    !is_wayland(),
                    "Capture Active Monitor is not supported on Wayland; choose a monitor or use --region screen<N>"
                );
                if let Ok((x, y)) = x11::pointer_position()
                    && let Some(index) = screens::at_point(monitors, f64::from(x), f64::from(y))
                {
                    selected_screen =
                        Some((available_screens.take().expect("available monitors"), index));
                }
            }
            let target_monitor = selected_screen.as_ref().map(|(_, index)| *index);
            let grid_origin = selected_screen
                .as_ref()
                .map(|(screens, index)| {
                    [
                        screens[*index].bounds.x as f32,
                        screens[*index].bounds.y as f32,
                    ]
                })
                .or_else(|| {
                    available_screens
                        .as_ref()
                        .and_then(|screens| screens.first())
                        .map(|screen| [screen.bounds.x as f32, screen.bounds.y as f32])
                })
                .unwrap_or([0.0; 2]);
            let screen_origins = available_screens
                .as_ref()
                .map(|screens| {
                    screens
                        .iter()
                        .map(|screen| [screen.bounds.x as f32, screen.bounds.y as f32])
                        .collect()
                })
                .unwrap_or_default();

            let connection = daemon::session_connection().await?;
            let capture = if settings.use_x11_legacy_screenshot && !is_wayland() {
                tokio::task::spawn_blocking(x11::capture_desktop).await??
            } else {
                match portal::screenshot(&connection, "", Duration::from_secs(15)).await {
                    Ok(path) => {
                        let reader = image::ImageReader::open(path)?.with_guessed_format()?;
                        reader
                            .decode()
                            .context("Cannot decode portal screenshot")?
                            .to_rgba8()
                    }
                    Err(error) if !is_wayland() && unavailable(&error) => {
                        eprintln!(
                            "flameshot: Screenshot portal unavailable, using direct X11 capture"
                        );
                        tokio::task::spawn_blocking(x11::capture_desktop).await??
                    }
                    Err(error) => return Err(error.into()),
                }
            };
            let screen_choices = if let Some(monitors) = &available_screens {
                monitors
                    .iter()
                    .enumerate()
                    .map(|(index, screen)| {
                        Ok((
                            screen.name.clone(),
                            screens::crop_bounds(
                                monitors,
                                index,
                                capture.width(),
                                capture.height(),
                            )?,
                        ))
                    })
                    .collect::<Result<Vec<_>>>()?
            } else {
                Vec::new()
            };
            let capture = if let Some((monitors, index)) = selected_screen {
                let bounds =
                    screens::crop_bounds(&monitors, index, capture.width(), capture.height())?;
                image::imageops::crop_imm(&capture, bounds.x, bounds.y, bounds.width, bounds.height)
                    .to_image()
            } else {
                capture
            };
            let size = [capture.width(), capture.height()];
            let initial_selection = match region_spec {
                Some(Spec::All) | Some(Spec::Screen(_)) => {
                    Some(flameshot_core::geometry::PixelRect {
                        x: 0,
                        y: 0,
                        width: size[0],
                        height: size[1],
                    })
                }
                Some(Spec::Rectangle(region)) => Some(
                    region
                        .clip(size)
                        .context("Requested region is outside the captured screen")?,
                ),
                None if last_region => region_cache::load()?.and_then(|region| region.clip(size)),
                None => None,
            };
            let outcome = if graphical {
                gui::edit(
                    capture,
                    gui::Options {
                        accept_on_select,
                        target_monitor,
                        grid_origin,
                        screen_origins,
                        screens: screen_choices,
                        initial_selection,
                        save_on_accept: path.is_some()
                            || (!raw && !clipboard && !requested_pin && !print_geometry),
                        save_path: path.clone(),
                    },
                )?
                .ok_or(portal::CaptureError::Cancelled)?
            } else {
                let selection = initial_selection.unwrap_or(flameshot_core::geometry::PixelRect {
                    x: 0,
                    y: 0,
                    width: capture.width(),
                    height: capture.height(),
                });
                let capture = image::imageops::crop_imm(
                    &capture,
                    selection.x,
                    selection.y,
                    selection.width,
                    selection.height,
                )
                .to_image();
                gui::Outcome {
                    image: capture,
                    selection,
                    action: gui::Action::Accept,
                    saved: None,
                }
            };
            if graphical
                && let Err(error) = Region::from_pixels(outcome.selection)
                    .map_err(anyhow::Error::from)
                    .and_then(region_cache::save)
            {
                eprintln!("flameshot: Cannot cache selection: {error}");
            }
            let settings = settings::Settings::load()?;
            let capture = outcome.image;
            let copy = clipboard || outcome.action == gui::Action::Copy;
            let pin = requested_pin || outcome.action == gui::Action::Pin;
            if pin {
                daemon::pin(&capture, outcome.selection).await?;
            }
            let mut saved = outcome.saved;
            if saved.is_none()
                && (path.is_some()
                    || outcome.action == gui::Action::Save
                    || (!raw
                        && !copy
                        && !pin
                        && !print_geometry
                        && outcome.action != gui::Action::External))
            {
                let path = path.unwrap_or_else(|| settings.save_directory());
                let saved_path = output::save_with_settings(&capture, &path, &settings)?;
                saved = Some(saved_path.clone());
                eprintln!("flameshot: Capture saved as {}", saved_path.display());
            }
            // Automatic saves happen before copying so the image remains the
            // final clipboard content even when copyPathAfterSave is enabled.
            if copy && settings.save_after_copy && saved.is_none() {
                match output::save_with_settings(&capture, &settings.save_directory(), &settings) {
                    Ok(path) => {
                        eprintln!("flameshot: Capture saved as {}", path.display());
                        saved = Some(path);
                    }
                    Err(error) => eprintln!("flameshot: Cannot save image after copy: {error}"),
                }
            }
            if settings.copy_path_after_save
                && !copy
                && let Some(path) = &saved
            {
                let absolute = std::fs::canonicalize(path).unwrap_or_else(|_| path.clone());
                daemon::copy_text(&absolute.to_string_lossy()).await?;
            }
            if copy {
                daemon::copy(&capture).await?;
            }
            if settings.notifications && (copy || pin || saved.is_some()) {
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

#[cfg(test)]
mod cli_tests {
    use super::*;
    #[test]
    fn legacy_shape_distinguishes_help_from_option_values() {
        for args in [
            vec!["flameshot", "--help"],
            vec!["flameshot", "gui", "-h"],
            vec!["flameshot", "--version"],
        ] {
            let error = parse_cli_from(args).err().unwrap();
            assert!(!error.use_stderr());
        }
        for args in [
            vec!["flameshot", "--help", "extra"],
            vec!["flameshot", "--version", "extra"],
            vec!["flameshot", "gui", "--help", "extra"],
            vec!["flameshot", "gui", "--raw", "--help"],
            vec!["flameshot", "full", "-rr"],
            vec!["flameshot", "full", "--raw=true"],
            vec!["flameshot", "full", "--"],
        ] {
            assert!(parse_cli_from(args).err().unwrap().use_stderr());
        }
        for args in [
            vec!["flameshot", "config", "--filename", "--help"],
            vec!["flameshot", "config", "--filename=", "--help"],
        ] {
            assert!(
                matches!(parse_cli_from(args).unwrap().command,Some(Command::Config {filename:Some(value),..}) if value=="--help")
            );
        }
        let error = parse_cli_from(["flameshot", "gui", "--delay", "bad", "--help"])
            .err()
            .unwrap();
        assert!(error.to_string().contains("2147483647"));
    }
    #[test]
    fn duplicate_options_keep_first_values_and_validate_all_occurrences() {
        let cli = parse_cli_from([
            "flameshot",
            "gui",
            "--delay",
            "5",
            "-d",
            "6",
            "--region",
            "all",
            "--region",
            "screen1",
            "--raw",
            "-r",
            "--pin",
            "--pin",
            "--last-region",
            "--last-region",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Gui {
                delay: 5,
                region: Some(flameshot_core::region::Spec::All),
                raw: true,
                pin: true,
                last_region: true,
                ..
            })
        ));
        let cli = parse_cli_from([
            "flameshot",
            "config",
            "--notifications",
            "false",
            "-n",
            "true",
            "--maincolor",
            "#123",
            "-m",
            "blue",
            "--check",
            "--check",
        ])
        .unwrap();
        assert!(
            matches!(cli.command,Some(Command::Config { notifications:Some(false),maincolor:Some(color),check:true,.. }) if color=="#112233")
        );
        for args in [
            vec!["flameshot", "full", "--delay", "1", "--delay", "2147483648"],
            vec!["flameshot", "screen", "--number", "0", "-n", "invalid"],
            vec!["flameshot", "gui", "--region", "all", "--region", "invalid"],
            vec![
                "flameshot",
                "config",
                "--notifications",
                "true",
                "-n",
                "invalid",
            ],
            vec![
                "flameshot",
                "config",
                "--maincolor",
                "red",
                "-m",
                "#11223344",
            ],
        ] {
            assert!(parse_cli_from(args).is_err());
        }
    }
    #[test]
    fn cli_colors_follow_qcolor_and_require_opacity() {
        for (source, expected) in [
            ("#123", "#112233"),
            ("#123456789", "#124578"),
            ("#008100000000", "#010000"),
            ("#ff7000000", "#ff0000"),
            ("#ff112233", "#112233"),
            (" dark blue ", "#00008b"),
        ] {
            assert_eq!(parse_cli_color(source).unwrap(), expected);
        }
        for source in ["#11223344", "transparent", " #123 ", "#12", "bogus"] {
            assert!(parse_cli_color(source).is_err(), "{source}");
        }
    }
    #[test]
    fn repeated_paths_preserve_first_non_utf8_path_and_reject_missing_parent() {
        use std::os::unix::ffi::OsStringExt;
        let root = tempfile::tempdir().unwrap();
        let first = root
            .path()
            .join(std::ffi::OsString::from_vec(b"image-\xff.png".to_vec()));
        let second = root.path().join("second.png");
        let args: Vec<std::ffi::OsString> = vec![
            "flameshot".into(),
            "full".into(),
            "--path".into(),
            first.clone().into(),
            "-p".into(),
            second.into(),
        ];
        assert!(
            matches!(parse_cli_from(args).unwrap().command,Some(Command::Full { path:Some(path),delay:0,.. }) if path==first)
        );
        let missing = root.path().join("absent/file.png");
        let args: Vec<std::ffi::OsString> = vec![
            "flameshot".into(),
            "gui".into(),
            "--path".into(),
            root.path().into(),
            "-p".into(),
            missing.into(),
        ];
        assert!(parse_cli_from(args).is_err());
    }
    #[test]
    fn numeric_options_keep_legacy_i32_bounds_and_whitespace() {
        for command in ["gui", "screen", "full"] {
            for (value, expected) in [("0", 0), (" +42 ", 42), ("2147483647", 2147483647)] {
                let cli = parse_cli_from(["flameshot", command, "--delay", value]).unwrap();
                let delay = match cli.command.unwrap() {
                    Command::Gui { delay, .. }
                    | Command::Screen { delay, .. }
                    | Command::Full { delay, .. } => delay,
                    _ => unreachable!(),
                };
                assert_eq!(delay, expected);
            }
            for value in ["2147483648", "18446744073709551615", "-1", "1.5", ""] {
                assert!(
                    parse_cli_from(["flameshot", command, &format!("--delay={value}")]).is_err()
                );
            }
        }
        for (value, expected) in [("0", 0), (" +42 ", 42), ("2147483647", 2147483647)] {
            let cli = parse_cli_from(["flameshot", "screen", "--number", value]).unwrap();
            assert!(
                matches!(cli.command, Some(Command::Screen { number: Some(number), .. }) if number == expected)
            );
        }
        for value in ["2147483648", "18446744073709551615", "-1", "1.5", ""] {
            assert!(parse_cli_from(["flameshot", "screen", &format!("--number={value}")]).is_err());
        }
    }
}
