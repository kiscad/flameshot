use crate::i18n::tr;
// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::Result;
use eframe::egui::{
    self, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions,
};
use flameshot_core::{
    editor::{Annotation, Document, Point, Style},
    geometry::PixelRect,
};
use image::RgbaImage;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, PartialEq)]
pub enum Action {
    Accept,
    Copy,
    Save,
    Pin,
}
pub struct Outcome {
    pub image: RgbaImage,
    pub selection: PixelRect,
    pub action: Action,
    pub saved: bool,
}
#[derive(Default, Clone)]
pub struct Options {
    pub accept_on_select: bool,
    pub save_on_accept: bool,
    pub save_path: Option<std::path::PathBuf>,
}
pub fn edit(image: RgbaImage, options: Options) -> Result<Option<Outcome>> {
    let result = Arc::new(Mutex::new(None));
    let app = Editor::new(image, result.clone(), options)?;
    eframe::run_native(
        "Flameshot",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_fullscreen(true)
                .with_decorations(false),
            ..Default::default()
        },
        Box::new(move |cc| {
            crate::i18n::install_fonts(&cc.egui_ctx);
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
    error: String,
    show_help: bool,
    selected_object: Option<usize>,
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
        Ok(Self {
            document: Document::new(image, 100)?,
            size,
            texture: None,
            dirty: true,
            selection: None,
            points: vec![],
            tool: Tool::Select,
            color: Color32::RED,
            width: settings.draw_thickness.min(10000.0),
            text: String::new(),
            font_size: (settings.font_size * 4.0 / 3.0).min(1000.0),
            error: String::new(),
            show_help: crate::settings::Settings::load()?.show_help,
            selected_object: None,
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
                .or(settings.save_path.clone())
                .or_else(dirs::picture_dir)
                .or_else(dirs::home_dir)
                .unwrap_or_else(|| ".".into());
            if explicit.is_some() && self.save_to(&path, ctx, action).is_ok() {
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
        self.finish(ctx, action, false);
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
                self.finish(ctx, action, true);
                Ok(())
            }
            Err(error) => {
                self.error = error.to_string();
                Err(error)
            }
        }
    }
    fn finish(&mut self, ctx: &egui::Context, action: Action, saved: bool) {
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
    fn pending_annotation(&self) -> Option<Annotation> {
        let start = *self.points.first()?;
        let end = *self.points.last()?;
        if self.points.len() < 2 {
            return None;
        }
        let region = selected_rect(start, end, self.size);
        let style = Style {
            color: self.color.to_array(),
            width: self.width,
        };
        match self.tool {
            Tool::Counter => Some(Annotation::Counter {
                center: start,
                tip: end,
                number: self.document.next_counter(),
                radius: self.width + 15.0,
                color: self.color.to_array(),
                outline: self.counter_outline,
            }),
            Tool::Pencil => Some(Annotation::Pencil {
                points: self.points.clone(),
                style,
            }),
            Tool::Line => Some(Annotation::Line { start, end, style }),
            Tool::Marker => Some(Annotation::Marker { start, end, style }),
            Tool::Arrow => Some(Annotation::Arrow { start, end, style }),
            Tool::Rectangle => Some(Annotation::Rectangle {
                start,
                end,
                style,
                filled: false,
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
                block_size: self.width.max(1.0) as u32,
            }),
            Tool::Select | Tool::Text | Tool::Move | Tool::MoveSelection => None,
        }
    }
    fn finish_stroke(&mut self) {
        if self.tool == Tool::Move {
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
        } else if let Some(annotation) = self.pending_annotation()
            && let Err(error) = self.document.add(annotation)
        {
            self.error = error.to_string();
        }
        self.points.clear();
        self.selection_drag = None;
        self.dirty = true;
    }
}
impl eframe::App for Editor {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        if self.save_dialog.is_some() {
            self.save_ui(ui);
            return;
        }
        let ctx = ui.ctx().clone();
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if !ctx.egui_wants_keyboard_input()
            && pressed_modifiers(&ctx, egui::Key::Z).is_some_and(|m| m.command)
        {
            self.dirty |= if pressed_modifiers(&ctx, egui::Key::Z).is_some_and(|m| m.shift) {
                self.document.redo()
            } else {
                self.document.undo()
            };
        }
        if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            self.accept(&ctx, Action::Accept);
        }
        if !ctx.egui_wants_keyboard_input() {
            for (key, tool) in [
                (egui::Key::P, Tool::Pencil),
                (egui::Key::M, Tool::Marker),
                (egui::Key::D, Tool::Line),
                (egui::Key::A, Tool::Arrow),
                (egui::Key::S, Tool::Select),
                (egui::Key::R, Tool::Rectangle),
                (egui::Key::C, Tool::Ellipse),
                (egui::Key::T, Tool::Text),
                (egui::Key::B, Tool::Pixelate),
                (egui::Key::I, Tool::Invert),
            ] {
                if pressed_modifiers(&ctx, key).is_some_and(|m| m.is_none()) {
                    self.tool = tool;
                }
            }
            if pressed_modifiers(&ctx, egui::Key::A).is_some_and(|m| m.command) {
                self.selection = Some(PixelRect {
                    x: 0,
                    y: 0,
                    width: self.size[0],
                    height: self.size[1],
                });
            }
        }
        if !ctx.egui_wants_keyboard_input() {
            if ctx.input(|i| {
                i.events
                    .iter()
                    .any(|event| matches!(event, egui::Event::Copy))
                    || (i.modifiers.command && i.key_pressed(egui::Key::C))
            }) {
                self.accept(&ctx, Action::Copy);
            }
            if ctx.input(|i| i.events.iter().any(|event| matches!(event,egui::Event::Key{key:egui::Key::S,pressed:true,modifiers,..} if modifiers.command))) {
                self.accept(&ctx, Action::Save);
            }
        }
        if !ctx.egui_wants_keyboard_input() {
            if ctx.input(|i| i.events.iter().any(|event| matches!(event,egui::Event::Key{key:egui::Key::M,pressed:true,modifiers,..} if modifiers.command))) {
                self.tool = Tool::MoveSelection;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Delete))
                && let Some(index) = self.selected_object.take()
            {
                if let Err(error) = self.document.remove(index) {
                    self.error = error.to_string();
                } else {
                    self.dirty = true;
                }
            }
        }
        if !ctx.egui_wants_keyboard_input() {
            for (key, dx, dy) in [
                (egui::Key::ArrowLeft, -1, 0),
                (egui::Key::ArrowRight, 1, 0),
                (egui::Key::ArrowUp, 0, -1),
                (egui::Key::ArrowDown, 0, 1),
            ] {
                if let Some(modifiers) = pressed_modifiers(&ctx, key)
                    && let Some(selection) = self.selection
                {
                    self.selection = flameshot_core::selection::keyboard(
                        selection,
                        dx,
                        dy,
                        modifiers.shift,
                        modifiers.command && modifiers.shift,
                        self.size,
                    );
                }
            }
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
                        for (tool, label) in [
                            (Tool::Select, "Select"),
                            (Tool::Text, "Text"),
                            (Tool::Pencil, "Pencil"),
                            (Tool::Marker, "Marker"),
                            (Tool::Counter, "Circle Counter"),
                            (Tool::Move, "Move object"),
                            (Tool::MoveSelection, "Move selection"),
                            (Tool::Line, "Line"),
                            (Tool::Arrow, "Arrow"),
                            (Tool::Rectangle, "Rectangle"),
                            (Tool::Ellipse, "Ellipse"),
                            (Tool::Invert, "Invert"),
                            (Tool::Pixelate, "Pixelate"),
                        ] {
                            let (context,source)=match tool {
 Tool::Select=>("SelectionTool","Rectangular Selection"),Tool::Text=>("TextTool",label),Tool::Pencil=>("PencilTool",label),Tool::Marker=>("MarkerTool",label),Tool::Counter=>("CircleCountTool",label),Tool::Move=>("MoveTool",label),Tool::MoveSelection=>("MoveTool",label),Tool::Line=>("LineTool",label),Tool::Arrow=>("ArrowTool",label),Tool::Rectangle=>("RectangleTool",label),Tool::Ellipse=>("CircleTool","Circle"),Tool::Invert=>("InvertTool",label),Tool::Pixelate=>("PixelateTool",label)};
 ui.selectable_value(&mut self.tool, tool, tr(context,source));
                        }
                        ui.color_edit_button_srgba(&mut self.color);
                        ui.add(egui::Slider::new(&mut self.width, 1.0..=40.0).text("Size"));
                        if ui.button(tr("UndoTool","Undo")).clicked() {
                            self.dirty |= self.document.undo();
                        }
                        if ui.button(tr("RedoTool","Redo")).clicked() {
                            self.dirty |= self.document.redo();
                        }
                        if ui.button(tr("CopyTool","Copy")).clicked() {
                            self.accept(&ctx, Action::Copy);
                        }
                        if ui.button(tr("SaveTool","Save")).clicked() {
                            self.accept(&ctx, Action::Save);
                        }
                        if ui.button(tr("PinTool","Pin Tool")).clicked() {
                            self.accept(&ctx, Action::Pin);
                        }
                        if ui.button(tr("AcceptTool","Accept")).clicked() {
                            self.accept(&ctx, Action::Accept);
                        }
                        if ui.button(tr("ColorGrabWidget","Cancel")).clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.checkbox(&mut self.magnifier,"Magnifier (G: pick color)");
                    if self.tool==Tool::Counter {ui.checkbox(&mut self.counter_outline,"Counter outline");}
                    if self.tool == Tool::Text {
                        ui.label("Enter text, then click the image to place it");
                        ui.text_edit_multiline(&mut self.text);
                        ui.add(
                            egui::Slider::new(&mut self.font_size, 6.0..=144.0).text("Font size"),
                        );
                    }
                    if !self.error.is_empty() {
                        ui.colored_label(Color32::RED, &self.error);
                    }
                });
            });
        if self.dirty {
            match self.document.preview(self.pending_annotation().as_ref()) {
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
            && let Some(pointer) = response.hover_pos()
            && let Some(rendered) = &self.rendered
        {
            let x = ((pointer.x - rect.min.x) / scale).floor().max(0.0) as u32;
            let y = ((pointer.y - rect.min.y) / scale).floor().max(0.0) as u32;
            if let Some(sample) = flameshot_core::inspection::sample(rendered, x, y, 7) {
                if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::G)) {
                    self.color = Color32::from_rgba_unmultiplied(
                        sample.rgba[0],
                        sample.rgba[1],
                        sample.rgba[2],
                        sample.rgba[3],
                    );
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
                number: self.document.next_counter(),
                radius: self.width + 15.0,
                color: self.color.to_array(),
                outline: self.counter_outline,
            }) {
                Ok(()) => self.dirty = true,
                Err(error) => self.error = error.to_string(),
            }
        }
        if response.clicked()
            && self.tool == Tool::Text
            && !self.text.is_empty()
            && let Some(position) = response.interact_pointer_pos()
        {
            let position = Point {
                x: (position.x - rect.min.x) / scale,
                y: (position.y - rect.min.y) / scale,
            };
            match self.document.add(Annotation::Text {
                position,
                text: self.text.clone(),
                font_size: self.font_size,
                family: String::new(),
                color: self.color.to_array(),
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
                        None
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
        if response.drag_stopped() {
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
            let (min, max) = annotation.bounds();
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
        if let Some(selection) = highlight {
            let min = rect.min + egui::vec2(selection.x as f32 * scale, selection.y as f32 * scale);
            let bounds = Rect::from_min_size(
                min,
                egui::vec2(
                    selection.width as f32 * scale,
                    selection.height as f32 * scale,
                ),
            );
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
                    ui.painter().rect_filled(handle, 0.0, Color32::WHITE);
                    ui.painter().rect_stroke(
                        handle,
                        0.0,
                        Stroke::new(1.0, Color32::BLACK),
                        StrokeKind::Inside,
                    );
                }
            }
            let shade = Color32::from_black_alpha(150);
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
                Stroke::new(1.0, Color32::WHITE),
                StrokeKind::Inside,
            );
        }
    }
}
fn pressed_modifiers(ctx: &egui::Context, key: egui::Key) -> Option<egui::Modifiers> {
    ctx.input(|i| {
        i.events.iter().find_map(|event| match event {
            egui::Event::Key {
                key: pressed,
                pressed: true,
                modifiers,
                ..
            } if *pressed == key => Some(*modifiers),
            _ => None,
        })
    })
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
        let mut output = ctx.run_ui(input, |ui| {
            assert!(
                pressed_modifiers(ui.ctx(), egui::Key::ArrowRight)
                    .unwrap()
                    .shift
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
