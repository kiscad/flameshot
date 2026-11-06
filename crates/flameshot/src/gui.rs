use crate::i18n::tr;
// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::Result;
use eframe::egui::{
    self, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions,
};
use flameshot_core::{
    editor::{Annotation, Document, Point, SizeKind, Style},
    geometry::PixelRect,
};
use image::RgbaImage;
use std::sync::{Arc, Mutex};

fn drawing_control_modifier(events: &[egui::Event], fallback: bool) -> bool {
    events
        .iter()
        .rev()
        .find_map(|event| {
            if let egui::Event::PointerButton {
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers,
                ..
            } = event
            {
                Some(modifiers.ctrl)
            } else {
                None
            }
        })
        .unwrap_or(fallback)
}

#[derive(Clone, Copy, PartialEq)]
pub enum Action {
    Accept,
    Copy,
    Save,
    Pin,
    External,
}
pub struct Outcome {
    pub image: RgbaImage,
    pub selection: PixelRect,
    pub action: Action,
    pub saved: Option<std::path::PathBuf>,
}
#[derive(Default, Clone)]
pub struct Options {
    pub target_monitor: Option<usize>,
    pub screens: Vec<(String, PixelRect)>,
    pub accept_on_select: bool,
    pub initial_selection: Option<PixelRect>,
    pub save_on_accept: bool,
    pub save_path: Option<std::path::PathBuf>,
}
pub fn edit(image: RgbaImage, options: Options) -> Result<Option<Outcome>> {
    let mut viewport = crate::app_ui::viewport()
        .with_fullscreen(true)
        .with_decorations(false);
    if let Some(index) = options.target_monitor {
        viewport = viewport.with_monitor(index);
    }
    let result = Arc::new(Mutex::new(None));
    let app = Editor::new(image, result.clone(), options)?;
    eframe::run_native(
        "Flameshot",
        eframe::NativeOptions {
            viewport,
            ..Default::default()
        },
        Box::new(move |cc| {
            crate::i18n::install_fonts(&cc.egui_ctx);
            app.theme.apply(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Cannot start editor: {e}"))?;
    let output = result
        .lock()
        .map_err(|_| anyhow::anyhow!("Editor result lock poisoned"))?
        .take();
    Ok(output)
}
#[derive(Clone, Copy, PartialEq)]
enum Tool {
    FilledRectangle,
    MoveSelection,
    Counter,
    Marker,
    Move,
    Text,
    Select,
    Pencil,
    Line,
    Arrow,
    Rectangle,
    Ellipse,
    Invert,
    Pixelate,
}
impl Tool {
    fn legacy_id(self) -> Option<i32> {
        match self {
            Self::Pencil => Some(0),
            Self::Line => Some(1),
            Self::Arrow => Some(2),
            Self::Rectangle => Some(3),
            Self::FilledRectangle => Some(4),
            Self::Ellipse => Some(5),
            Self::Marker => Some(6),
            Self::MoveSelection => Some(8),
            Self::Pixelate => Some(15),
            Self::Text => Some(18),
            Self::Counter => Some(19),
            Self::Invert => Some(22),
            Self::Select | Self::Move => None,
        }
    }
}
struct FileEntry {
    name: String,
    path: std::path::PathBuf,
    directory: bool,
}
type FolderResult = Result<Vec<FileEntry>, String>;
struct SaveDialog {
    focus: bool,
    listed_folder: Option<std::path::PathBuf>,
    pending_folder: Option<std::path::PathBuf>,
    listing_task: Option<std::sync::mpsc::Receiver<FolderResult>>,
    entries: Vec<FileEntry>,
    folder_error: String,
    path: String,
    error: String,
    action: Action,
}
impl SaveDialog {
    fn refresh_folder(&mut self, folder: &std::path::Path, ctx: &egui::Context) {
        if let Some(receiver) = &self.listing_task
            && let Ok(result) = receiver.try_recv()
        {
            self.listed_folder = self.pending_folder.take();
            self.listing_task = None;
            match result {
                Ok(entries) => {
                    self.entries = entries;
                    self.folder_error.clear();
                }
                Err(error) => {
                    self.entries.clear();
                    self.folder_error = error;
                }
            }
        }
        if self.listing_task.is_none() && self.listed_folder.as_deref() != Some(folder) {
            self.entries.clear();
            self.folder_error.clear();
            let folder = folder.to_owned();
            self.pending_folder = Some(folder.clone());
            let (sender, receiver) = std::sync::mpsc::channel();
            self.listing_task = Some(receiver);
            std::thread::spawn(move || {
                let result = std::fs::read_dir(&folder)
                    .map(|entries| {
                        let mut entries: Vec<_> = entries
                            .filter_map(Result::ok)
                            .map(|entry| {
                                let path = entry.path();
                                let directory = path.is_dir();
                                FileEntry {
                                    name: entry.file_name().to_string_lossy().into_owned(),
                                    path,
                                    directory,
                                }
                            })
                            .collect();
                        entries
                            .sort_by(|a, b| (!a.directory, &a.name).cmp(&(!b.directory, &b.name)));
                        entries
                    })
                    .map_err(|e| e.to_string());
                let _ = sender.send(result);
            });
        }
        if self.listing_task.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
    }
}
struct Editor {
    show_quit_prompt: bool,
    quit_prompt: bool,
    quit_focus: bool,
    quit_do_not_show: bool,
    allow_close: bool,
    insecure_pixelate: bool,
    geometry_overlay: crate::geometry_overlay::GeometryOverlay,
    geometry_mode: u8,
    geometry_timeout: u64,
    show_panel_button: bool,
    palette: Vec<Option<[u8; 4]>>,
    dismissed_dialog_frame: Option<u64>,
    text_editor: Option<crate::text_edit::TextEdit>,
    theme: crate::theme::Theme,
    copy_on_double_click: bool,
    contrast_opacity: u8,
    wheel: crate::wheel::ToolWheel,
    wheel_frame: Option<u64>,
    counter_override: Option<u16>,
    undo_limit: usize,
    marker_size: f32,
    pixelate_size: f32,
    counter_size: f32,
    curved_arrow: bool,
    reverse_arrow: bool,
    rectangle_radius: f32,
    open_with: Option<crate::open_with::OpenWith>,
    shortcuts: crate::shortcuts::Shortcuts,
    panel_visible: bool,
    buttons: Option<Vec<i32>>,
    screen_image: Option<RgbaImage>,
    screen_previews: Vec<TextureHandle>,
    #[cfg(feature = "imgur")]
    upload_dialog: Option<crate::upload_ui::UploadDialog>,
    document: Document,
    size: [u32; 2],
    texture: Option<TextureHandle>,
    dirty: bool,
    selection: Option<PixelRect>,
    points: Vec<Point>,
    tool: Tool,
    color: Color32,
    width: f32,
    text: String,
    font_size: f32,
    font_family: String,
    error: String,
    show_help: bool,
    selected_object: Option<usize>,
    property_pointer_down: bool,
    constrain_drawing: bool,
    counter_outline: bool,
    rendered: Option<RgbaImage>,
    magnifier: bool,
    square_magnifier: bool,
    magnifier_texture: Option<TextureHandle>,
    selection_drag: Option<(PixelRect, flameshot_core::selection::Drag)>,
    options: Options,
    save_dialog: Option<SaveDialog>,
    result: Arc<Mutex<Option<Outcome>>>,
}
impl Editor {
    fn new(
        image: RgbaImage,
        result: Arc<Mutex<Option<Outcome>>>,
        options: Options,
    ) -> Result<Self> {
        let size = [image.width(), image.height()];
        let settings = crate::settings::Settings::load()?;
        let screen_image = (options.screens.len() > 1).then(|| image.clone());
        Ok(Self {
            show_quit_prompt: settings.show_quit_prompt,
            quit_prompt: false,
            quit_focus: false,
            quit_do_not_show: false,
            allow_close: false,
            insecure_pixelate: settings.insecure_pixelate,
            geometry_overlay: Default::default(),
            geometry_mode: settings.selection_geometry,
            geometry_timeout: settings.selection_geometry_timeout,
            show_panel_button: settings.show_side_panel_button,
            palette: settings.palette.clone(),
            dismissed_dialog_frame: None,
            text_editor: None,
            theme: crate::theme::Theme::from_settings(&settings),
            copy_on_double_click: settings.copy_on_double_click,
            contrast_opacity: settings.contrast_opacity,
            wheel: Default::default(),
            wheel_frame: None,
            counter_override: None,
            undo_limit: settings.undo_limit,
            marker_size: settings.marker_size.min(10000.0),
            pixelate_size: settings.pixelate_size.min(10000.0),
            counter_size: settings.counter_size.min(985.0),
            curved_arrow: settings.arrow_style == 1,
            reverse_arrow: settings.reverse_arrow,
            rectangle_radius: settings.rectangle_radius,
            open_with: None,
            shortcuts: crate::shortcuts::Shortcuts::new(&settings.shortcuts),
            panel_visible: false,
            buttons: settings.buttons.clone(),
            screen_image,
            screen_previews: Vec::new(),
            #[cfg(feature = "imgur")]
            upload_dialog: None,
            document: Document::new(image, settings.undo_limit)?,
            size,
            texture: None,
            dirty: true,
            selection: options.initial_selection,
            points: vec![],
            tool: Tool::Select,
            color: Color32::from_rgba_unmultiplied(
                settings.draw_color[0],
                settings.draw_color[1],
                settings.draw_color[2],
                settings.draw_color[3],
            ),
            font_family: settings.font_family.clone(),
            width: settings.draw_thickness.min(10000.0),
            text: String::new(),
            font_size: (settings.font_size * 4.0 / 3.0).min(1000.0),
            error: String::new(),
            show_help: crate::settings::Settings::load()?.show_help,
            selected_object: None,
            property_pointer_down: false,
            constrain_drawing: false,
            counter_outline: settings.counter_outline,
            rendered: None,
            magnifier: settings.magnifier,
            square_magnifier: settings.square_magnifier,
            magnifier_texture: None,
            selection_drag: None,
            options,
            save_dialog: None,
            result,
        })
    }
    fn choose_screen(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self.screen_previews.is_empty() {
            let image = self.screen_image.as_ref().unwrap();
            for (index, (_, r)) in self.options.screens.iter().enumerate() {
                let crop = image::imageops::crop_imm(image, r.x, r.y, r.width, r.height).to_image();
                let thumb = image::imageops::thumbnail(&crop, 300, 180);
                self.screen_previews.push(ctx.load_texture(
                    format!("screen-{index}"),
                    egui::ColorImage::from_rgba_unmultiplied(
                        [thumb.width() as usize, thumb.height() as usize],
                        thumb.as_raw(),
                    ),
                    Default::default(),
                ));
            }
        }
        let mut chosen = None;
        egui::Window::new("Select a monitor")
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(&ctx, |ui| {
                ui.label("Choose a screen to edit. Number keys 1–9 select a screen.");
                ui.horizontal_wrapped(|ui| {
                    for (index, (name, _)) in self.options.screens.iter().enumerate() {
                        ui.vertical(|ui| {
                            ui.image(&self.screen_previews[index]);
                            if ui.button(format!("{}: {name}", index + 1)).clicked() {
                                chosen = Some(index);
                            }
                        });
                    }
                });
            });
        for (index, key) in [
            egui::Key::Num1,
            egui::Key::Num2,
            egui::Key::Num3,
            egui::Key::Num4,
            egui::Key::Num5,
            egui::Key::Num6,
            egui::Key::Num7,
            egui::Key::Num8,
            egui::Key::Num9,
        ]
        .into_iter()
        .enumerate()
        {
            if index < self.options.screens.len() && ctx.input(|i| i.key_pressed(key)) {
                chosen = Some(index);
            }
        }
        if let Some(index) = chosen {
            let r = self.options.screens[index].1;
            let image = self.screen_image.take().unwrap();
            let crop = image::imageops::crop_imm(&image, r.x, r.y, r.width, r.height).to_image();
            self.size = [crop.width(), crop.height()];
            self.selection = self
                .options
                .initial_selection
                .and_then(|r| flameshot_core::region::Region::from_pixels(r).ok())
                .and_then(|r| r.clip(self.size));
            self.document = Document::new(crop, self.undo_limit).expect("validated monitor crop");
            self.texture = None;
            self.rendered = None;
            self.dirty = true;
            self.screen_previews.clear();
            ctx.send_viewport_cmd(egui::ViewportCommand::SetMonitor(index));
            ctx.request_repaint();
        }
    }
    fn open_application(&mut self) {
        if let Some(selection) = self.selection {
            match self.document.export(selection) {
                Ok(image) => {
                    let keep = crate::settings::Settings::load()
                        .map(|s| s.keep_open_app_launcher)
                        .unwrap_or(false);
                    self.open_with = Some(crate::open_with::OpenWith::new(image, keep));
                }
                Err(error) => self.error = error.to_string(),
            }
        } else {
            self.error = "Select an area first".into();
        }
    }
    fn request_quit(&mut self, ctx: &egui::Context) {
        if self.show_quit_prompt && !self.allow_close {
            if !self.quit_prompt {
                self.quit_focus = true;
            }
            self.quit_prompt = true;
            ctx.request_repaint();
        } else {
            self.allow_close = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
    fn quit_ui(&mut self, ui: &egui::Ui) {
        let ctx = ui.ctx();
        if let Some(texture) = &self.texture {
            ui.painter().image(
                texture.id(),
                ctx.content_rect(),
                Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        let mut cancel = ctx.input(|input| input.key_pressed(egui::Key::Escape));
        let mut confirm = false;
        egui::Window::new(tr("CaptureWidget", "Quit Capture"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(tr(
                    "CaptureWidget",
                    "Are you sure you want to quit capture?",
                ));
                if ui
                    .checkbox(
                        &mut self.quit_do_not_show,
                        tr("CaptureWidget", "Do not show this again"),
                    )
                    .changed()
                {
                    match crate::settings::Settings::update(&[(
                        "showQuitPrompt",
                        (!self.quit_do_not_show).to_string(),
                    )]) {
                        Ok(()) => self.show_quit_prompt = !self.quit_do_not_show,
                        Err(error) => self.error = error.to_string(),
                    }
                }
                ui.horizontal(|ui| {
                    let no = ui.button("No");
                    if self.quit_focus {
                        no.request_focus();
                        self.quit_focus = false;
                    }
                    cancel |= no.clicked();
                    confirm |= ui.button("Yes").clicked();
                });
                if !self.error.is_empty() {
                    ui.colored_label(Color32::RED, &self.error);
                }
            });
        if confirm {
            self.allow_close = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else if cancel {
            self.quit_prompt = false;
            self.dismissed_dialog_frame = Some(ctx.cumulative_frame_nr());
            self.shortcuts.reset();
            ctx.input_mut(|input| {
                input.consume_key(egui::Modifiers::NONE, egui::Key::Escape);
                input.consume_key(egui::Modifiers::NONE, egui::Key::Enter);
            });
            ctx.request_repaint();
        }
    }
    fn button_enabled(&self, id: i32) -> bool {
        self.buttons
            .as_ref()
            .map_or(!matches!(id, 20 | 21), |buttons| buttons.contains(&id))
    }
    fn accept(&mut self, ctx: &egui::Context, action: Action) {
        if self.selection.is_none() {
            self.error = "Select an area first".into();
            return;
        }
        let should_save = action == Action::Save
            || (self.options.save_on_accept
                && (action == Action::Accept || self.options.save_path.is_some()));
        if should_save {
            let settings = match crate::settings::Settings::load() {
                Ok(s) => s,
                Err(e) => {
                    self.error = e.to_string();
                    return;
                }
            };
            let explicit = self.options.save_path.clone();
            let path = explicit
                .clone()
                .unwrap_or_else(|| settings.save_directory());
            if (explicit.is_some() || settings.save_path_fixed)
                && self.save_to(&path, ctx, action).is_ok()
            {
                return;
            }
            let path = if path.is_dir() {
                path.join(format!(
                    "{}.{}",
                    chrono::Local::now().format(&settings.filename_pattern),
                    settings.extension
                ))
            } else {
                path
            };
            ctx.request_repaint();
            self.save_dialog = Some(SaveDialog {
                focus: true,
                listed_folder: None,
                pending_folder: None,
                listing_task: None,
                entries: Vec::new(),
                folder_error: String::new(),
                path: path.to_string_lossy().into_owned(),
                error: self.error.clone(),
                action,
            });
            return;
        }
        self.finish(ctx, action, None);
    }
    fn save_to(
        &mut self,
        path: &std::path::Path,
        ctx: &egui::Context,
        action: Action,
    ) -> Result<()> {
        let selection = self
            .selection
            .ok_or_else(|| anyhow::anyhow!("Select an area first"))?;
        let image = self.document.export(selection)?;
        let settings = crate::settings::Settings::load()?;
        match crate::output::save_with_settings(&image, path, &settings) {
            Ok(saved) => {
                eprintln!("flameshot: Capture saved as {}", saved.display());
                self.finish(ctx, action, Some(saved));
                Ok(())
            }
            Err(error) => {
                self.error = error.to_string();
                Err(error)
            }
        }
    }
    fn finish(&mut self, ctx: &egui::Context, action: Action, saved: Option<std::path::PathBuf>) {
        if let Some(selection) = self.selection {
            match self.document.export(selection) {
                Ok(image) => {
                    if let Ok(mut result) = self.result.lock() {
                        *result = Some(Outcome {
                            image,
                            selection,
                            action,
                            saved,
                        });
                    }
                    self.allow_close = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Err(error) => self.error = error.to_string(),
            }
        } else {
            self.error = "Select an area first".into();
        }
    }
    fn save_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if let Some(texture) = &self.texture {
            ui.painter().image(
                texture.id(),
                ctx.content_rect(),
                Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        let mut dialog = self.save_dialog.take().unwrap();
        let mut cancel = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let mut save = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        egui::Window::new("Save Screenshot")
            .collapsible(false)
            .resizable(true)
            .default_width(550.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(&ctx, |ui| {
                let original = std::path::Path::new(&dialog.path)
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("png")
                    .to_ascii_lowercase();
                let mut extension = original.clone();
                egui::ComboBox::from_label("Image format")
                    .selected_text(&extension)
                    .show_ui(ui, |ui| {
                        for &value in crate::output::EXTENSIONS {
                            ui.selectable_value(&mut extension, value.into(), value.to_uppercase());
                        }
                    });
                if extension != original {
                    let mut path = std::path::PathBuf::from(&dialog.path);
                    if path.is_dir() {
                        path = path.join("capture");
                    }
                    path.set_extension(extension);
                    dialog.path = path.to_string_lossy().into_owned();
                }
                ui.label("Destination file");
                let id = egui::Id::unique("save-destination");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut dialog.path)
                        .id(id)
                        .desired_width(f32::INFINITY),
                );
                if dialog.focus {
                    response.request_focus();
                    dialog.focus = false;
                }
                let path = std::path::PathBuf::from(&dialog.path);
                let folder = if path.is_dir() {
                    path.clone()
                } else {
                    path.parent()
                        .unwrap_or(std::path::Path::new("."))
                        .to_owned()
                };
                if ui.button("Parent folder").clicked()
                    && let Some(parent) = folder.parent()
                {
                    dialog.path = parent.to_string_lossy().into_owned();
                }
                dialog.refresh_folder(&folder, &ctx);
                if !dialog.folder_error.is_empty() {
                    ui.colored_label(Color32::RED, &dialog.folder_error);
                }
                let row_height =
                    ui.text_style_height(&egui::TextStyle::Body) + ui.spacing().item_spacing.y;
                egui::ScrollArea::vertical().max_height(250.0).show_rows(
                    ui,
                    row_height,
                    dialog.entries.len(),
                    |ui, range| {
                        for index in range {
                            let entry = &dialog.entries[index];
                            let label = if entry.directory {
                                format!("{}/", entry.name)
                            } else {
                                entry.name.clone()
                            };
                            if ui.selectable_label(false, label).clicked() {
                                dialog.path = entry.path.to_string_lossy().into_owned();
                            }
                        }
                    },
                );
                if !dialog.error.is_empty() {
                    ui.colored_label(Color32::RED, &dialog.error);
                }
                ui.horizontal(|ui| {
                    save |= ui.button(tr("SaveTool", "Save")).clicked();
                    cancel |= ui.button(tr("ColorGrabWidget", "Cancel")).clicked();
                });
            });
        if cancel {
            return;
        }
        if save {
            match self.save_to(std::path::Path::new(&dialog.path), &ctx, dialog.action) {
                Ok(()) => return,
                Err(error) => {
                    dialog.error = error.to_string();
                    dialog.focus = true;
                }
            }
        }
        self.save_dialog = Some(dialog);
    }
    fn selected_size(&self) -> Option<(SizeKind, f32)> {
        if self.tool != Tool::Move {
            return None;
        }
        self.selected_object
            .and_then(|index| self.document.annotations().get(index))
            .and_then(Annotation::tool_size)
    }
    fn tool_size(&mut self) -> (&'static str, &mut f32, f32) {
        let tool = if let Some((kind, value)) = self.selected_size() {
            match kind {
                SizeKind::Stroke => {
                    self.width = value;
                    Tool::Pencil
                }
                SizeKind::Marker => {
                    self.marker_size = value;
                    Tool::Marker
                }
                SizeKind::Text => {
                    self.font_size = value / 0.75;
                    Tool::Text
                }
                SizeKind::Counter => {
                    self.counter_size = value;
                    Tool::Counter
                }
                SizeKind::CornerRadius => {
                    self.rectangle_radius = value;
                    Tool::FilledRectangle
                }
                SizeKind::Pixelate => {
                    self.pixelate_size = value;
                    Tool::Pixelate
                }
            }
        } else {
            self.tool
        };
        match tool {
            Tool::Marker => ("drawMarkerSize", &mut self.marker_size, 1.0),
            Tool::Pixelate => ("drawPixelateSize", &mut self.pixelate_size, 1.0),
            Tool::Counter => ("drawCircleCounterSize", &mut self.counter_size, 1.0),
            Tool::FilledRectangle => ("drawRectangleSize", &mut self.rectangle_radius, 1.0),
            Tool::Text => ("drawFontSize", &mut self.font_size, 0.75),
            _ => ("drawThickness", &mut self.width, 1.0),
        }
    }
    fn adjust_tool_size(&mut self, delta: f32) {
        if self.tool == Tool::Move
            && self.selected_object.is_some()
            && self.selected_size().is_none()
        {
            return;
        }
        let (key, size, factor) = self.tool_size();
        let limit = match key {
            "drawFontSize" => 750.0,
            "drawCircleCounterSize" => 985.0,
            _ => 10000.0,
        };
        let value = (*size * factor + delta).round().clamp(1.0, limit);
        *size = value / factor;
        self.persist_tool_size(key, value);
    }
    fn palette_ui(&mut self, ui: &mut egui::Ui, close_on_select: bool) {
        ui.horizontal_wrapped(|ui| {
            for entry in self.palette.clone() {
                if let Some([r, g, b, a]) = entry {
                    let color = Color32::from_rgba_unmultiplied(r, g, b, a);
                    if ui
                        .add(
                            egui::Button::new(" ")
                                .fill(color)
                                .min_size(egui::vec2(22.0, 22.0)),
                        )
                        .on_hover_text(flameshot_core::color::encode([r, g, b, a]))
                        .clicked()
                    {
                        self.color = color;
                        self.persist_color();
                        if close_on_select {
                            ui.close();
                        }
                    }
                } else if ui.color_edit_button_srgba(&mut self.color).changed() {
                    self.persist_color();
                }
            }
        });
    }
    fn persist_color(&mut self) {
        self.dirty = true;
        if self.tool == Tool::Move
            && let Some(index) = self.selected_object
            && let Some(original) = self.document.annotations().get(index)
        {
            let mut annotation = original.clone();
            annotation.set_color(self.color.to_srgba_unmultiplied());
            if self.property_pointer_down {
                self.document.begin_transaction();
            }
            if let Err(error) = self.document.replace(index, annotation) {
                self.error = error.to_string();
            }
        }
        if let Err(error) = crate::settings::Settings::update(&[(
            "drawColor",
            flameshot_core::color::encode(self.color.to_srgba_unmultiplied()),
        )]) {
            self.error = error.to_string();
        }
    }
    fn persist_tool_size(&mut self, key: &str, value: f32) {
        self.dirty = true;
        if self.tool == Tool::Move
            && let Some(index) = self.selected_object
            && let Some(annotation) = self.document.annotations().get(index)
        {
            match annotation.with_tool_size(value).and_then(|next| {
                if self.property_pointer_down {
                    self.document.begin_transaction();
                }
                self.document.replace(index, next)
            }) {
                Ok(()) => {}
                Err(error) => {
                    self.error = error.to_string();
                    return;
                }
            }
        }
        if let Err(error) =
            crate::settings::Settings::update(&[(key, (value.round() as u32).to_string())])
        {
            self.error = error.to_string();
        }
    }
    fn tool_size_control(&mut self, ui: &mut egui::Ui) {
        if self.tool == Tool::Move
            && self.selected_object.is_some()
            && self.selected_size().is_none()
        {
            return;
        }
        let (key, size, factor) = self.tool_size();
        let mut value = *size * factor;
        let limit = match key {
            "drawFontSize" => 750.0,
            "drawCircleCounterSize" => 985.0,
            _ => 10000.0,
        };
        if ui
            .add(
                egui::DragValue::new(&mut value)
                    .range(1.0..=limit)
                    .speed(1.0)
                    .prefix("Size: "),
            )
            .changed()
        {
            value = value.round();
            *size = value / factor;
            self.persist_tool_size(key, value);
        }
    }
    fn pending_annotation(&self) -> Option<Annotation> {
        if self.selection_drag.is_some() {
            return None;
        }
        let start = *self.points.first()?;
        let end = *self.points.last()?;
        let end = if self.constrain_drawing {
            use flameshot_core::constraints::{DrawingConstraint, endpoint};
            match self.tool {
                Tool::Line | Tool::Arrow | Tool::Marker => {
                    endpoint(start, end, DrawingConstraint::AxisAndDiagonal)
                }
                Tool::Rectangle | Tool::FilledRectangle | Tool::Ellipse => {
                    endpoint(start, end, DrawingConstraint::Diagonal)
                }
                _ => end,
            }
        } else {
            end
        };
        if self.points.len() < 2 {
            return None;
        }
        let region = selected_rect(start, end, self.size);
        let style = Style {
            color: self.color.to_srgba_unmultiplied(),
            width: self.width,
        };
        match self.tool {
            Tool::Counter => Some(Annotation::Counter {
                center: start,
                tip: end,
                number: self
                    .counter_override
                    .unwrap_or_else(|| self.document.next_counter()),
                radius: self.counter_size + 15.0,
                color: self.color.to_srgba_unmultiplied(),
                outline: self.counter_outline,
            }),
            Tool::Pencil => Some(Annotation::Pencil {
                points: self.points.clone(),
                style,
            }),
            Tool::Line => Some(Annotation::Line { start, end, style }),
            Tool::Marker => Some(Annotation::Marker {
                start,
                end,
                style: Style {
                    width: self.marker_size,
                    ..style
                },
            }),
            Tool::Arrow => Some(Annotation::Arrow {
                start,
                end,
                style,
                curved: self.curved_arrow,
                reversed: self.reverse_arrow,
            }),
            Tool::Rectangle | Tool::FilledRectangle => Some(Annotation::Rectangle {
                start,
                end,
                style,
                filled: self.tool == Tool::FilledRectangle,
                radius: if self.tool == Tool::FilledRectangle {
                    self.rectangle_radius
                } else {
                    0.0
                },
            }),
            Tool::Ellipse => Some(Annotation::Ellipse {
                start,
                end,
                style,
                filled: false,
            }),
            Tool::Invert => region.map(Annotation::Invert),
            Tool::Pixelate => region.map(|region| Annotation::Pixelate {
                region,
                block_size: (2.0 * (self.pixelate_size + 1.0)) as u32,
                insecure: self.insecure_pixelate,
            }),
            Tool::Select | Tool::Text | Tool::Move | Tool::MoveSelection => None,
        }
    }
    fn undo_edit(&mut self) {
        self.dirty |= self.document.undo();
        self.selected_object = None;
        self.counter_override = None;
    }
    fn redo_edit(&mut self) {
        self.dirty |= self.document.redo();
        self.selected_object = None;
        self.counter_override = None;
    }
    fn advance_counter(&mut self) {
        if let Some(number) = &mut self.counter_override {
            *number = number.saturating_add(1).min(999);
        }
    }
    fn pending_move(&self) -> Option<(usize, Point)> {
        if self.tool != Tool::Move || self.selection_drag.is_some() {
            return None;
        }
        let start = self.points.first()?;
        let end = self.points.last()?;
        Some((
            self.selected_object?,
            Point {
                x: end.x - start.x,
                y: end.y - start.y,
            },
        ))
    }
    fn preview(&self) -> Result<RgbaImage, flameshot_core::editor::EditError> {
        if let Some((index, delta)) = self.pending_move() {
            self.document.preview_move(index, delta.x, delta.y)
        } else {
            self.document.preview(self.pending_annotation().as_ref())
        }
    }
    fn finish_stroke(&mut self) {
        if self.selection_drag.is_some() {
            // The selection was already updated during the drag.
        } else if self.tool == Tool::Move {
            if let (Some(index), Some(start), Some(end)) = (
                self.selected_object,
                self.points.first(),
                self.points.last(),
            ) && let Err(error) =
                self.document
                    .move_annotation(index, end.x - start.x, end.y - start.y)
            {
                self.error = error.to_string();
            }
        } else if self.tool == Tool::Select && self.selection_drag.is_none() {
            if let (Some(start), Some(end)) = (self.points.first(), self.points.last()) {
                self.selection = selected_rect(*start, *end, self.size);
            }
        } else if let Some(annotation) = self.pending_annotation() {
            match self.document.add(annotation) {
                Ok(()) => {
                    if self.tool == Tool::Counter {
                        self.advance_counter();
                    }
                }
                Err(error) => self.error = error.to_string(),
            }
        }
        self.points.clear();
        self.selection_drag = None;
        self.dirty = true;
    }
}
impl eframe::App for Editor {
    fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        if self.show_quit_prompt && !self.allow_close && take_close_request(input) {
            self.request_quit(ctx);
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        // Flush one frame after release so final release-frame changes belong
        // to the same gesture, even when the next input starts another gesture.
        if !self.property_pointer_down {
            self.document.finish_transaction();
        }
        let pointer_down = ui.input(|input| input.pointer.primary_down());
        if self.property_pointer_down && !pointer_down {
            ui.ctx().request_repaint();
        }
        self.property_pointer_down = pointer_down;
        if self.quit_prompt {
            self.quit_ui(ui);
            return;
        }
        if self.screen_image.is_some() {
            self.choose_screen(ui);
            return;
        }
        #[cfg(feature = "imgur")]
        if let Some(mut dialog) = self.upload_dialog.take() {
            self.shortcuts.reset();
            if dialog.show(ui.ctx()) {
                self.upload_dialog = Some(dialog);
            }
            return;
        }
        if let Some(mut dialog) = self.open_with.take() {
            self.shortcuts.reset();
            if dialog.show(ui.ctx()) {
                self.open_with = Some(dialog);
            } else if dialog.was_launched() {
                self.accept(ui.ctx(), Action::External);
            }
            return;
        }
        if let Some(mut editor) = self.text_editor.take() {
            editor.paint_preview(ui, &self.document);
            let commit = self
                .shortcuts
                .actions(ui.ctx())
                .iter()
                .any(|action| action == "TYPE_COMMIT_CURRENT_TOOL");
            match editor.show(ui.ctx(), &mut self.document, commit) {
                None => self.text_editor = Some(editor),
                Some(_applied) => {
                    self.dismissed_dialog_frame = Some(ui.ctx().cumulative_frame_nr());
                    self.dirty = true;
                    ui.ctx().request_repaint();
                    self.selected_object = None;
                    self.shortcuts.reset();
                }
            }
            return;
        }
        if self.save_dialog.is_some() {
            self.shortcuts.reset();
            self.save_ui(ui);
            return;
        }
        let ctx = ui.ctx().clone();
        let constrain = ctx.input(|input| {
            if self.points.is_empty() {
                input.modifiers.ctrl
            } else {
                drawing_control_modifier(&input.events, input.modifiers.ctrl)
            }
        });
        if self.constrain_drawing != constrain {
            self.constrain_drawing = constrain;
            if self.points.len() >= 2
                && matches!(
                    self.tool,
                    Tool::Line
                        | Tool::Arrow
                        | Tool::Marker
                        | Tool::Rectangle
                        | Tool::FilledRectangle
                        | Tool::Ellipse
                )
            {
                self.dirty = true;
                ctx.request_repaint();
            }
        }
        if self.dismissed_dialog_frame != Some(ctx.cumulative_frame_nr())
            && ctx.input(|i| i.key_pressed(egui::Key::Escape))
        {
            if self.tool != Tool::Select {
                self.tool = Tool::Select;
                self.points.clear();
                self.selection_drag = None;
                self.dirty = true;
            } else if self.selected_object.take().is_some() {
            } else if self.panel_visible {
                self.panel_visible = false;
            } else {
                self.request_quit(&ctx);
            }
            if self.quit_prompt {
                return;
            }
        }
        let mut grab_color = false;
        let actions = if self.dismissed_dialog_frame == Some(ctx.cumulative_frame_nr()) {
            Vec::new()
        } else {
            self.shortcuts.actions(&ctx)
        };
        if ctx.egui_wants_keyboard_input() {
            self.shortcuts.reset_sequences();
        }
        if !ctx.egui_wants_keyboard_input() {
            for action in actions {
                if self.quit_prompt {
                    break;
                }
                match action.as_str() {
                    "TYPE_PENCIL" => self.tool = Tool::Pencil,
                    "TYPE_DRAWER" => self.tool = Tool::Line,
                    "TYPE_ARROW" => self.tool = Tool::Arrow,
                    "TYPE_SELECTION" => self.tool = Tool::Rectangle,
                    "TYPE_RECTANGLE" => self.tool = Tool::FilledRectangle,
                    "TYPE_CIRCLE" => self.tool = Tool::Ellipse,
                    "TYPE_MARKER" => self.tool = Tool::Marker,
                    "TYPE_MOVESELECTION" => self.tool = Tool::MoveSelection,
                    "TYPE_PIXELATE" => self.tool = Tool::Pixelate,
                    "TYPE_INVERT" => self.tool = Tool::Invert,
                    "TYPE_TEXT" => self.tool = Tool::Text,
                    "TYPE_CIRCLECOUNT" => self.tool = Tool::Counter,
                    "TYPE_UNDO" => self.undo_edit(),
                    "TYPE_REDO" => self.redo_edit(),
                    "TYPE_COPY" => self.accept(&ctx, Action::Copy),
                    "TYPE_SAVE" => self.accept(&ctx, Action::Save),
                    "TYPE_PIN" => self.accept(&ctx, Action::Pin),
                    "TYPE_OPEN_APP" => self.open_application(),
                    #[cfg(feature = "imgur")]
                    "TYPE_IMAGEUPLOADER" => {
                        if let Some(selection) = self.selection {
                            match self.document.export(selection) {
                                Ok(image) => {
                                    self.upload_dialog =
                                        Some(crate::upload_ui::UploadDialog::new(image))
                                }
                                Err(error) => self.error = error.to_string(),
                            }
                        } else {
                            self.error = "Select an area first".into();
                        }
                    }
                    "TYPE_ACCEPT" => self.accept(&ctx, Action::Accept),
                    "TYPE_EXIT" => self.request_quit(&ctx),
                    "TYPE_CANCEL" => {
                        self.points.clear();
                        self.selection = None;
                        self.tool = Tool::Select;
                        self.dirty = true;
                    }
                    "TYPE_SELECT_ALL" => {
                        self.selection = Some(PixelRect {
                            x: 0,
                            y: 0,
                            width: self.size[0],
                            height: self.size[1],
                        })
                    }
                    "TYPE_SIZEINCREASE" => self.adjust_tool_size(1.0),
                    "TYPE_SIZEDECREASE" => self.adjust_tool_size(-1.0),
                    "TYPE_GRAB_COLOR" => grab_color = true,
                    "TYPE_TOGGLE_PANEL" => self.panel_visible = !self.panel_visible,
                    "TYPE_DELETE_CURRENT_TOOL" => {
                        if let Some(index) = self.selected_object.take() {
                            let counter = matches!(
                                self.document.annotations().get(index),
                                Some(Annotation::Counter { .. })
                            );
                            if let Err(error) = self.document.remove(index) {
                                self.error = error.to_string();
                            } else {
                                if counter {
                                    self.counter_override = self
                                        .counter_override
                                        .map(|number| number.saturating_sub(1).max(1));
                                }
                                self.dirty = true;
                            }
                        }
                    }
                    "TYPE_COMMIT_CURRENT_TOOL" => {
                        if !self.points.is_empty() {
                            self.finish_stroke();
                        }
                    }
                    name if name.starts_with("TYPE_MOVE_")
                        || name.starts_with("TYPE_RESIZE_")
                        || name.starts_with("TYPE_SYM_RESIZE_") =>
                    {
                        let direction = name.rsplit('_').next().unwrap_or("");
                        let (dx, dy) = match direction {
                            "LEFT" => (-1, 0),
                            "RIGHT" => (1, 0),
                            "UP" => (0, -1),
                            "DOWN" => (0, 1),
                            _ => (0, 0),
                        };
                        if let Some(selection) = self.selection {
                            self.selection = flameshot_core::selection::keyboard(
                                selection,
                                dx,
                                dy,
                                name.contains("RESIZE"),
                                name.contains("SYM"),
                                self.size,
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
        if self.quit_prompt {
            return;
        }
        if self.tool == Tool::Move
            && let Some(color) = self
                .selected_object
                .and_then(|index| self.document.annotations().get(index))
                .and_then(Annotation::color)
        {
            self.color = Color32::from_rgba_unmultiplied(color[0], color[1], color[2], color[3]);
        }
        if self.panel_visible {
            egui::Window::new("Objects and properties")
                .default_pos(egui::pos2(20.0, 180.0))
                .show(&ctx, |ui| {
                    self.tool_size_control(ui);
                    if ui.color_edit_button_srgba(&mut self.color).changed() {
                        self.persist_color();
                    }
                    ui.horizontal(|ui| {
                        let selected = self
                            .selected_object
                            .filter(|index| *index < self.document.annotations().len());
                        let mut target = None;
                        if ui
                            .add_enabled(
                                selected.is_some_and(|index| index > 0),
                                egui::Button::new(tr("AbstractWidgetList", "Move Up")),
                            )
                            .clicked()
                        {
                            target = selected.map(|index| (index, index - 1));
                        }
                        if ui
                            .add_enabled(
                                selected.is_some_and(|index| {
                                    index + 1 < self.document.annotations().len()
                                }),
                                egui::Button::new(tr("AbstractWidgetList", "Move Down")),
                            )
                            .clicked()
                        {
                            target = selected.map(|index| (index, index + 1));
                        }
                        if let Some((from, to)) = target {
                            match self.document.move_layer(from, to) {
                                Ok(()) => {
                                    self.selected_object = Some(to);
                                    self.dirty = true;
                                }
                                Err(error) => self.error = error.to_string(),
                            }
                        }
                    });
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .show(ui, |ui| {
                            for (index, annotation) in
                                self.document.annotations().iter().enumerate()
                            {
                                let label = match annotation {
                                    Annotation::Text { .. } => "Text",
                                    Annotation::Counter { .. } => "Counter",
                                    Annotation::Pencil { .. } => "Pencil",
                                    Annotation::Line { .. } => "Line",
                                    Annotation::Marker { .. } => "Marker",
                                    Annotation::Arrow { .. } => "Arrow",
                                    Annotation::Rectangle { .. } => "Rectangle",
                                    Annotation::Ellipse { .. } => "Ellipse",
                                    Annotation::Invert(_) => "Invert",
                                    Annotation::Pixelate { .. } => "Pixelate",
                                };
                                if ui
                                    .selectable_label(
                                        self.selected_object == Some(index),
                                        format!("{}: {label}", index + 1),
                                    )
                                    .clicked()
                                {
                                    self.selected_object = Some(index);
                                    self.tool = Tool::Move;
                                }
                            }
                        });
                });
        }
        egui::Area::new(egui::Id::unique("capture-tools"))
            .order(egui::Order::Foreground)
            .default_pos(egui::pos2(20.0, 20.0))
            .movable(true)
            .show(&ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    if self.show_help && self.selection.is_none() {ui.label("Drag to select • Enter: accept • Ctrl+C: copy • Ctrl+S: save • Esc: cancel");}
                    ui.set_max_width((ctx.content_rect().width() - 40.0).clamp(200.0, 900.0));
                    ui.horizontal_wrapped(|ui| {
                        let mut tools=vec![
                            (Tool::Select, "Select"),
                            (Tool::Text, "Text"),
                            (Tool::Pencil, "Pencil"),
                            (Tool::Marker, "Marker"),
                            (Tool::Counter, "Circle Counter"),
                            (Tool::Move, "Move object"),
                            (Tool::MoveSelection, "Move selection"),
                            (Tool::Line, "Line"),
                            (Tool::Arrow, "Arrow"),
                            (Tool::Rectangle, "Outline rectangle"),
                            (Tool::FilledRectangle, "Rectangle"),
                            (Tool::Ellipse, "Ellipse"),
                            (Tool::Invert, "Invert"),
                            (Tool::Pixelate, "Pixelate"),
                        ];
                        if let Some(buttons)=&self.buttons {
                            tools.retain(|(tool,_)|tool.legacy_id().is_none_or(|id|buttons.contains(&id)));
                        }
                            tools.sort_by_key(|(tool,_)|tool.legacy_id().map(|id|crate::buttons::priority(id)+1).unwrap_or(0));
                        for (tool,label) in tools {
                            let (context,source)=match tool {
 Tool::Select=>("CaptureWidget","Capture region"),Tool::Text=>("TextTool",label),Tool::Pencil=>("PencilTool",label),Tool::Marker=>("MarkerTool",label),Tool::Counter=>("CircleCountTool",label),Tool::Move=>("MoveTool",label),Tool::MoveSelection=>("MoveTool",label),Tool::Line=>("LineTool",label),Tool::Arrow=>("ArrowTool",label),Tool::Rectangle=>("SelectionTool","Rectangular Selection"),Tool::FilledRectangle=>("RectangleTool",label),Tool::Ellipse=>("CircleTool","Circle"),Tool::Invert=>("InvertTool",label),Tool::Pixelate=>("PixelateTool",label)};
 ui.selectable_value(&mut self.tool, tool, tr(context,source));
                        }
                        if ui.color_edit_button_srgba(&mut self.color).changed() { self.persist_color(); }
                        self.tool_size_control(ui);
                        for (id,context,label) in [
                            (9,"UndoTool","Undo"),(16,"RedoTool","Redo"),(10,"CopyTool","Copy"),(11,"SaveTool","Save"),
                            (13,"ImageUploaderTool","Upload"),(23,"AcceptTool","Accept"),(14,"AppLauncher","Open with"),
                            (12,"ColorGrabWidget","Cancel"),(17,"PinTool","Pin Tool"),(20,"CaptureWidget","+"),(21,"CaptureWidget","−"),
                        ] {
                            if self.button_enabled(id) && crate::buttons::available(id) && ui.button(tr(context,label)).clicked() {
                                match id {
                                    9 => self.undo_edit(),
                                    16 => self.redo_edit(),
                                    10 => self.accept(&ctx,Action::Copy),
                                    11 => self.accept(&ctx,Action::Save),
                                    #[cfg(feature="imgur")]
                                    13 => {
                                        if let Some(selection)=self.selection { match self.document.export(selection) { Ok(image)=>self.upload_dialog=Some(crate::upload_ui::UploadDialog::new(image)),Err(error)=>self.error=error.to_string() } }
                                        else { self.error="Select an area first".into(); }
                                    }
                                    23 => self.accept(&ctx,Action::Accept),
                                    14 => self.open_application(),
                                    12 => self.request_quit(&ctx),
                                    17 => self.accept(&ctx,Action::Pin),
                                    20 => self.adjust_tool_size(1.0),
                                    21 => self.adjust_tool_size(-1.0),
                                    _ => {}
                                }
                            }
                        }
                    });
                    self.palette_ui(ui,false);
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.magnifier,"Magnifier (G: pick color)");
                        if self.show_panel_button && ui.button("Objects").clicked() {self.panel_visible = !self.panel_visible;}
                    });
                    if self.tool==Tool::Arrow {
                        let changed=ui.checkbox(&mut self.curved_arrow,"Curved arrow head").changed() | ui.checkbox(&mut self.reverse_arrow,"Reverse arrow").changed();
                        if changed && let Err(error)=crate::settings::Settings::update(&[("arrowStyle",u8::from(self.curved_arrow).to_string()),("reverseArrow",self.reverse_arrow.to_string())]) {self.error=error.to_string();}
                    }

                    if self.tool==Tool::Counter {ui.checkbox(&mut self.counter_outline,"Counter outline");}
                    if self.tool == Tool::Text {
                        ui.label("Click the image to enter text, or type here to place repeated labels");
                        ui.text_edit_multiline(&mut self.text);
                        ui.horizontal(|ui| {
                            ui.label("Font family");
                            if ui.text_edit_singleline(&mut self.font_family).changed() && let Err(error) = crate::settings::Settings::update(&[("fontFamily", self.font_family.clone())]) { self.error = error.to_string(); }
                        });

                    }
                    if !self.error.is_empty() {
                        ui.colored_label(Color32::RED, &self.error);
                    }
                });
            });
        if self.dirty {
            match self.preview() {
                Ok(image) => {
                    let pixels = egui::ColorImage::from_rgba_unmultiplied(
                        [self.size[0] as usize, self.size[1] as usize],
                        image.as_raw(),
                    );
                    if let Some(texture) = &mut self.texture {
                        texture.set(pixels, TextureOptions::NEAREST);
                    } else {
                        self.texture =
                            Some(ctx.load_texture("capture", pixels, TextureOptions::NEAREST));
                    }
                    self.rendered = Some(image);
                    self.dirty = false;
                }
                Err(e) => self.error = e.to_string(),
            }
        }
        let Some(texture) = &self.texture else {
            return;
        };
        let available = ctx.content_rect();
        let scale = (available.width() / self.size[0] as f32)
            .min(available.height() / self.size[1] as f32)
            .max(0.001);
        let rect = Rect::from_center_size(
            available.center(),
            egui::vec2(self.size[0] as f32 * scale, self.size[1] as f32 * scale),
        );
        let response = ui.interact(
            rect,
            egui::Id::unique("capture-canvas"),
            Sense::click_and_drag(),
        );
        ui.painter().image(
            texture.id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        if response.hovered()
            && !ctx.egui_wants_keyboard_input()
            && self.wheel_frame != Some(ctx.cumulative_frame_nr())
        {
            // egui can run multiple layout passes using the same input events.
            self.wheel_frame = Some(ctx.cumulative_frame_nr());
            for event in ctx.input(|input| input.events.clone()) {
                if let egui::Event::MouseWheel {
                    unit,
                    delta,
                    modifiers,
                    ..
                } = event
                {
                    let step = self.wheel.step(unit, delta.y, std::time::Instant::now());
                    if step == 0 {
                        continue;
                    }
                    if self.tool == Tool::Counter && modifiers.ctrl {
                        let number = self
                            .counter_override
                            .unwrap_or_else(|| self.document.next_counter());
                        self.counter_override =
                            Some((i32::from(number) + step).clamp(1, 999) as u16);
                    } else {
                        self.adjust_tool_size(step as f32);
                    }
                    self.dirty = true;
                    ctx.request_repaint();
                    // Coalesce duplicate wheel notifications within one frame.
                    break;
                }
            }
        }
        if response.hovered()
            && let Some(pointer) = response.hover_pos()
            && let Some(rendered) = &self.rendered
        {
            let x = ((pointer.x - rect.min.x) / scale).floor().max(0.0) as u32;
            let y = ((pointer.y - rect.min.y) / scale).floor().max(0.0) as u32;
            if let Some(sample) = flameshot_core::inspection::sample(rendered, x, y, 7) {
                if grab_color {
                    self.color = Color32::from_rgba_unmultiplied(
                        sample.rgba[0],
                        sample.rgba[1],
                        sample.rgba[2],
                        sample.rgba[3],
                    );
                    if let Err(error) = crate::settings::Settings::update(&[(
                        "drawColor",
                        flameshot_core::color::encode(sample.rgba),
                    )]) {
                        self.error = error.to_string();
                    }
                }
                if self.magnifier {
                    let r = sample.neighborhood;
                    let crop =
                        image::imageops::crop_imm(rendered, r.x, r.y, r.width, r.height).to_image();
                    let mut enlarged = image::imageops::resize(
                        &crop,
                        r.width * 10,
                        r.height * 10,
                        image::imageops::FilterType::Nearest,
                    );
                    if !self.square_magnifier {
                        let center = egui::vec2(
                            enlarged.width() as f32 / 2.0,
                            enlarged.height() as f32 / 2.0,
                        );
                        let radius = center.x.min(center.y);
                        for (px, py, pixel) in enlarged.enumerate_pixels_mut() {
                            if (egui::vec2(px as f32 + 0.5, py as f32 + 0.5) - center).length_sq()
                                > radius * radius
                            {
                                pixel[3] = 0;
                            }
                        }
                    }
                    let pixels = egui::ColorImage::from_rgba_unmultiplied(
                        [enlarged.width() as usize, enlarged.height() as usize],
                        enlarged.as_raw(),
                    );
                    if let Some(texture) = &mut self.magnifier_texture {
                        texture.set(pixels, TextureOptions::NEAREST);
                    } else {
                        self.magnifier_texture =
                            Some(ctx.load_texture("magnifier", pixels, TextureOptions::NEAREST));
                    }
                    let texture = self.magnifier_texture.as_ref().unwrap();
                    let position = egui::pos2(
                        (pointer.x + 24.0)
                            .min(available.max.x - 180.0)
                            .max(available.min.x),
                        (pointer.y + 24.0)
                            .min(available.max.y - 190.0)
                            .max(available.min.y),
                    );
                    egui::Area::new(egui::Id::unique("magnifier-overlay"))
                        .order(egui::Order::Tooltip)
                        .fixed_pos(position)
                        .interactable(false)
                        .show(&ctx, |ui| {
                            let (zoom, _) = ui.allocate_exact_size(
                                egui::vec2(enlarged.width() as f32, enlarged.height() as f32),
                                Sense::hover(),
                            );
                            ui.painter().image(
                                texture.id(),
                                zoom,
                                Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
                                Color32::WHITE,
                            );
                            if self.square_magnifier {
                                ui.painter().rect_stroke(
                                    zoom,
                                    0.0,
                                    Stroke::new(1.0, Color32::WHITE),
                                    StrokeKind::Inside,
                                );
                            } else {
                                ui.painter().circle_stroke(
                                    zoom.center(),
                                    zoom.width().min(zoom.height()) / 2.0,
                                    Stroke::new(1.0, Color32::WHITE),
                                );
                            }
                            let pixel = Rect::from_min_size(
                                zoom.min
                                    + egui::vec2(
                                        sample.center[0] as f32 * 10.0,
                                        sample.center[1] as f32 * 10.0,
                                    ),
                                egui::vec2(10.0, 10.0),
                            );
                            ui.painter().rect_stroke(
                                pixel,
                                0.0,
                                Stroke::new(1.0, Color32::RED),
                                StrokeKind::Inside,
                            );
                            ui.label(format!(
                                "#{:02X}{:02X}{:02X}  {}, {}",
                                sample.rgba[0], sample.rgba[1], sample.rgba[2], x, y
                            ));
                        });
                }
            }
        }
        response.context_menu(|ui| {
            self.palette_ui(ui, true);
        });
        if response.double_clicked_by(egui::PointerButton::Primary)
            && let Some(position) = response.interact_pointer_pos()
            && let Some(index) = self.document.hit_test(Point {
                x: (position.x - rect.min.x) / scale,
                y: (position.y - rect.min.y) / scale,
            })
            && let Some(editor) = crate::text_edit::TextEdit::new(&self.document, index)
        {
            self.text_editor = Some(editor);
            self.points.clear();
            self.selection_drag = None;
            self.shortcuts.reset();
            ctx.request_repaint();
            return;
        }
        if self.copy_on_double_click
            && response.double_clicked_by(egui::PointerButton::Primary)
            && let Some(position) = response.interact_pointer_pos()
            && let Some(selection) = self.selection
        {
            let point = Point {
                x: (position.x - rect.min.x) / scale,
                y: (position.y - rect.min.y) / scale,
            };
            if point.x >= selection.x as f32
                && point.y >= selection.y as f32
                && point.x < (selection.x + selection.width) as f32
                && point.y < (selection.y + selection.height) as f32
                && self.document.hit_test(point).is_none()
            {
                self.accept(&ctx, Action::Copy);
                return;
            }
        }
        if response.clicked()
            && self.tool == Tool::Counter
            && let Some(position) = response.interact_pointer_pos()
        {
            let center = Point {
                x: (position.x - rect.min.x) / scale,
                y: (position.y - rect.min.y) / scale,
            };
            match self.document.add(Annotation::Counter {
                center,
                tip: center,
                number: self
                    .counter_override
                    .unwrap_or_else(|| self.document.next_counter()),
                radius: self.counter_size + 15.0,
                color: self.color.to_srgba_unmultiplied(),
                outline: self.counter_outline,
            }) {
                Ok(()) => {
                    self.advance_counter();
                    self.dirty = true;
                }
                Err(error) => self.error = error.to_string(),
            }
        }
        if response.clicked()
            && self.tool == Tool::Text
            && let Some(position) = response.interact_pointer_pos()
        {
            let position = Point {
                x: (position.x - rect.min.x) / scale,
                y: (position.y - rect.min.y) / scale,
            };
            if self.document.hit_test(position).is_some_and(|index| {
                matches!(self.document.annotations()[index], Annotation::Text { .. })
            }) {
                return;
            }
            if self.text.is_empty() {
                self.text_editor = crate::text_edit::TextEdit::create(Annotation::Text {
                    position,
                    text: String::new(),
                    font_size: self.font_size,
                    family: self.font_family.clone(),
                    color: self.color.to_srgba_unmultiplied(),
                });
                self.shortcuts.reset();
                ctx.request_repaint();
                return;
            }
            match self.document.add(Annotation::Text {
                position,
                text: self.text.clone(),
                font_size: self.font_size,
                family: self.font_family.clone(),
                color: self.color.to_srgba_unmultiplied(),
            }) {
                Ok(()) => self.dirty = true,
                Err(error) => self.error = error.to_string(),
            }
        }
        if response.clicked()
            && self.tool == Tool::Move
            && let Some(position) = response.interact_pointer_pos()
        {
            self.selected_object = self.document.hit_test(Point {
                x: (position.x - rect.min.x) / scale,
                y: (position.y - rect.min.y) / scale,
            });
        }
        if response.drag_started() {
            self.selection_drag = None;
            self.points.clear();
            if let Some(position) = ctx.input(|i| i.pointer.press_origin()) {
                if let Some(selection) = self.selection {
                    let mode = if self.tool == Tool::MoveSelection {
                        Some(flameshot_core::selection::Drag::Move)
                    } else if self.tool == Tool::Select {
                        flameshot_core::selection::hit(
                            selection,
                            (position.x - rect.min.x) / scale,
                            (position.y - rect.min.y) / scale,
                            6.0 / scale,
                        )
                    } else {
                        match flameshot_core::selection::hit(
                            selection,
                            (position.x - rect.min.x) / scale,
                            (position.y - rect.min.y) / scale,
                            6.0 / scale,
                        ) {
                            Some(mode @ flameshot_core::selection::Drag::Resize(_)) => Some(mode),
                            _ => None,
                        }
                    };
                    self.selection_drag = mode.map(|mode| (selection, mode));
                }
                if self.tool == Tool::Move {
                    self.selected_object = self.document.hit_test(Point {
                        x: (position.x - rect.min.x) / scale,
                        y: (position.y - rect.min.y) / scale,
                    });
                }
                self.points.push(Point {
                    x: ((position.x - rect.min.x) / scale).clamp(0.0, self.size[0] as f32),
                    y: ((position.y - rect.min.y) / scale).clamp(0.0, self.size[1] as f32),
                });
            }
        }
        if (response.dragged() || response.drag_stopped())
            && !self.points.is_empty()
            && let Some(position) = response.interact_pointer_pos()
        {
            self.dirty = true;
            ctx.request_repaint();
            self.points.push(Point {
                x: ((position.x - rect.min.x) / scale).clamp(0.0, self.size[0] as f32),
                y: ((position.y - rect.min.y) / scale).clamp(0.0, self.size[1] as f32),
            });
        }
        if let Some((original, mode)) = self.selection_drag
            && let (Some(start), Some(end)) = (self.points.first(), self.points.last())
        {
            self.selection = flameshot_core::selection::drag(
                original,
                mode,
                (end.x - start.x).round() as i64,
                (end.y - start.y).round() as i64,
                self.size,
            );
        }
        if response.drag_stopped() && !self.points.is_empty() {
            self.finish_stroke();
            if self.tool == Tool::Select
                && self.selection.is_some()
                && self.options.accept_on_select
            {
                self.accept(&ctx, Action::Accept);
            }
        }
        if let Some(annotation) = self
            .selected_object
            .and_then(|index| self.document.annotations().get(index))
        {
            let moved = self.pending_move().and_then(|(index, delta)| {
                self.document.moved_annotation(index, delta.x, delta.y).ok()
            });
            let (min, max) = moved.as_ref().unwrap_or(annotation).bounds();
            ui.painter().rect_stroke(
                Rect::from_min_max(
                    rect.min + egui::vec2(min.x * scale, min.y * scale),
                    rect.min + egui::vec2(max.x * scale, max.y * scale),
                ),
                0.0,
                Stroke::new(1.0, Color32::YELLOW),
                StrokeKind::Outside,
            );
        }
        let highlight = if self.tool == Tool::Select
            && self.selection_drag.is_none()
            && self.points.len() > 1
        {
            selected_rect(self.points[0], *self.points.last().unwrap(), self.size)
        } else {
            self.selection
        };
        let mut overlay_bounds = None;
        if let Some(selection) = highlight {
            let min = rect.min + egui::vec2(selection.x as f32 * scale, selection.y as f32 * scale);
            let bounds = Rect::from_min_size(
                min,
                egui::vec2(
                    selection.width as f32 * scale,
                    selection.height as f32 * scale,
                ),
            );
            overlay_bounds = Some(bounds);
            if matches!(self.tool, Tool::Select | Tool::MoveSelection) {
                for point in [
                    bounds.left_top(),
                    bounds.center_top(),
                    bounds.right_top(),
                    bounds.right_center(),
                    bounds.right_bottom(),
                    bounds.center_bottom(),
                    bounds.left_bottom(),
                    bounds.left_center(),
                ] {
                    let handle = Rect::from_center_size(point, egui::vec2(6.0, 6.0));
                    ui.painter().rect_filled(handle, 0.0, self.theme.main);
                    ui.painter().rect_stroke(
                        handle,
                        0.0,
                        Stroke::new(1.0, Color32::BLACK),
                        StrokeKind::Inside,
                    );
                }
            }
            let shade = Color32::from_black_alpha(self.contrast_opacity);
            for outside in [
                Rect::from_min_max(rect.min, egui::pos2(rect.max.x, bounds.min.y)),
                Rect::from_min_max(egui::pos2(rect.min.x, bounds.max.y), rect.max),
                Rect::from_min_max(
                    egui::pos2(rect.min.x, bounds.min.y),
                    egui::pos2(bounds.min.x, bounds.max.y),
                ),
                Rect::from_min_max(
                    egui::pos2(bounds.max.x, bounds.min.y),
                    egui::pos2(rect.max.x, bounds.max.y),
                ),
            ] {
                ui.painter().rect_filled(outside, 0.0, shade);
            }
            ui.painter().rect_stroke(
                bounds,
                0.0,
                Stroke::new(1.0, self.theme.main),
                StrokeKind::Inside,
            );
        }
        self.geometry_overlay.show(
            ui,
            highlight,
            overlay_bounds,
            self.geometry_mode,
            self.geometry_timeout,
        );
    }
}
fn take_close_request(input: &mut egui::RawInput) -> bool {
    let Some(viewport) = input.viewports.get_mut(&input.viewport_id) else {
        return false;
    };
    let requested = viewport.close_requested();
    viewport
        .events
        .retain(|event| !matches!(event, egui::ViewportEvent::Close));
    requested
}
fn selected_rect(a: Point, b: Point, size: [u32; 2]) -> Option<PixelRect> {
    let x = a.x.min(b.x).max(0.0).floor() as u32;
    let y = a.y.min(b.y).max(0.0).floor() as u32;
    let end_x = (a.x.max(b.x).ceil() as u32).min(size[0]);
    let end_y = (a.y.max(b.y).ceil() as u32).min(size[1]);
    (end_x > x && end_y > y).then(|| PixelRect {
        x,
        y,
        width: end_x - x,
        height: end_y - y,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn drawing_keeps_mouse_release_modifiers_when_control_is_released_in_same_frame() {
        let events = [egui::Event::PointerButton {
            pos: egui::pos2(10.0, 20.0),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers {
                ctrl: true,
                ..Default::default()
            },
        }];
        assert!(drawing_control_modifier(&events, false));
        assert!(!drawing_control_modifier(&[], false));
        assert!(drawing_control_modifier(&[], true));
    }
    #[test]
    fn native_close_is_consumed_once_before_eframe_can_replay_it() {
        let mut input = egui::RawInput::default();
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .events
            .push(egui::ViewportEvent::Close);
        assert!(take_close_request(&mut input));
        assert!(!take_close_request(&mut input));
    }
    #[test]
    fn shortcuts_keep_press_modifiers_after_release_in_same_frame() {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            events: vec![
                egui::Event::Key {
                    key: egui::Key::ArrowRight,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers {
                        shift: true,
                        ..Default::default()
                    },
                },
                egui::Event::Key {
                    key: egui::Key::ArrowRight,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers: Default::default(),
                },
            ],
            ..Default::default()
        };
        let mut shortcuts = crate::shortcuts::Shortcuts::new(&Default::default());
        let mut output = ctx.run_ui(input, |ui| {
            assert!(
                shortcuts
                    .actions(ui.ctx())
                    .contains(&"TYPE_RESIZE_RIGHT".into())
            );
        });
        output.textures_delta.clear();
    }
    #[test]
    fn reverse_drag_and_fractional_edges_cover_selected_pixels() {
        assert_eq!(
            selected_rect(Point { x: 9.5, y: 8.5 }, Point { x: 1.2, y: 2.2 }, [10, 10]),
            Some(PixelRect {
                x: 1,
                y: 2,
                width: 9,
                height: 7
            })
        );
        assert_eq!(
            selected_rect(Point { x: 2.0, y: 2.0 }, Point { x: 2.0, y: 2.0 }, [10, 10]),
            None
        );
    }
}
