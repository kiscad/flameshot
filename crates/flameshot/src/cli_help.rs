// SPDX-License-Identifier: GPL-3.0-or-later
//! Localize presentation without changing argument identifiers, parsers or actions.
use crate::i18n::tr;
pub(crate) fn localize(mut command: clap::Command) -> clap::Command {
    let about = match command.get_name() {
        "flameshot" => Some(tr(
            "QObject",
            "Powerful yet simple to use screenshot software.",
        )),
        "gui" => Some(tr("QObject", "Start a manual capture in GUI mode.")),
        "screen" => Some(tr(
            "QObject",
            "Capture a screenshot of the specified monitor.",
        )),
        "full" => Some(tr(
            "QObject",
            "Capture screenshot of all monitors at the same time.",
        )),
        "launcher" => Some(tr("QObject", "Open the capture launcher.")),
        "config" => Some(tr("QObject", "Configure")),
        "history" => Some(tr("UploadHistory", "Upload History")),
        "updates" => Some(tr("TrayIcon", "Check for updates")),
        "help" => Some(tr("RustCli", "Show command help")),
        _ => None,
    };
    if command.get_name() == "flameshot" {
        command = command.after_help(tr(
            "QObject",
            "Per default runs Flameshot in the background and adds a tray icon for configuration.",
        ));
    }
    if let Some(about) = about {
        command = command.about(about);
    }
    command
        .help_template(format!("{{about-with-newline}}\n{}: {{usage}}\n\n{{all-args}}{{after-help}}", tr("QObject", "Usage")))
        .subcommand_help_heading(tr("QObject", "Arguments"))
        .mut_args(|argument| {
            let description = match argument.get_id().as_str() {
                "help" => Some(tr("RustCli", "Show help")),
                "_version" => Some(tr("RustCli", "Display version information")),
                "path" => Some(tr("QObject", "Existing directory or new file to save to")),
                "clipboard" => Some(tr("QObject", "Save the capture to the clipboard")),
                "pin" => Some(tr("QObject", "Pin the capture to the screen")),
                "delay" => Some(tr("QObject", "Delay time in milliseconds")),
                "region" => Some(tr("QObject", "Screenshot region to select")),
                "last_region" => Some(tr("QObject", "Repeat screenshot with previously selected region")),
                "filename" => Some(tr("QObject", "Set the filename pattern")),
                "accept_on_select" => Some(tr("QObject", "Accept capture as soon as a selection is made")),
                "trayicon" => Some(tr("QObject", "Enable or disable the trayicon")),
                "autostart" => Some(tr("QObject", "Enable or disable run at startup")),
                "notifications" => Some(tr("QObject", "Enable or disable the notifications")),
                "check" => Some(tr("QObject", "Check the configuration for errors")),
                "showhelp" => Some(tr("QObject", "Show the help message in the capture mode")),
                "maincolor" => Some(tr("QObject", "Define the main UI color")),
                "contrastcolor" => Some(tr("QObject", "Define the contrast UI color")),
                "raw" => Some(tr("QObject", "Print raw PNG capture")),
                "print_geometry" => Some(tr("QObject", "Print geometry of the selection in the format WxH+X+Y. Does nothing if raw is specified")),
                "edit" => Some(tr("QObject", "Interactively select and edit the screenshot region")),
                _ => None,
            };
            let heading = if argument.is_positional() { tr("QObject", "Arguments") } else { tr("QObject", "Options") };
            let mut argument = argument.help_heading(heading);
            if argument.get_id() == "number" {
                argument = argument.help(format!("{}\n{} (X11)", tr("QObject", "Define the screen to capture (starting from 0)"), tr("QObject", "default: screen containing the cursor")));
            } else if let Some(description) = description {
                argument = argument.help(description);
            }
            argument
        })
        .mut_subcommands(localize)
}
