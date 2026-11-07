// SPDX-License-Identifier: GPL-3.0-or-later
use crate::i18n::tr;
use eframe::egui;
use flameshot_core::editor::{Annotation, Document, EditError, TextAlignment, TextStyle};

pub(crate) fn format_controls(ui: &mut egui::Ui, format: &mut TextStyle) {
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(&mut format.bold, tr("TextConfig", "Bold"));
        ui.checkbox(&mut format.italic, tr("TextConfig", "Italic"));
        ui.checkbox(&mut format.underline, tr("TextConfig", "Underline"));
        ui.checkbox(&mut format.strikeout, tr("TextConfig", "StrikeOut"));
    });
    ui.horizontal_wrapped(|ui| {
        for (alignment, label) in [
            (TextAlignment::Left, "Left Align"),
            (TextAlignment::Center, "Center Align"),
            (TextAlignment::Right, "Right Align"),
        ] {
            ui.selectable_value(&mut format.alignment, alignment, tr("TextConfig", label));
        }
    });
}

pub struct TextEdit {
    previewed: Option<Annotation>,
    preview_texture: Option<egui::TextureHandle>,
    index: Option<usize>,
    original: Annotation,
    draft: Annotation,
    focus: bool,
    composing: bool,
    error: String,
}
impl TextEdit {
    pub fn creation_defaults(&self) -> Option<(f32, String, TextStyle)> {
        if self.index.is_some() {
            return None;
        }
        match &self.draft {
            Annotation::Text {
                text,
                font_size,
                family,
                format,
                ..
            } if !text.is_empty() => Some((*font_size, family.clone(), *format)),
            _ => None,
        }
    }
    pub fn new(document: &Document, index: usize) -> Option<Self> {
        let original = document.annotations().get(index)?.clone();
        if !matches!(original, Annotation::Text { .. }) {
            return None;
        }
        Some(Self {
            previewed: None,
            preview_texture: None,
            index: Some(index),
            draft: original.clone(),
            original,
            focus: true,
            composing: false,
            error: String::new(),
        })
    }
    pub fn create(draft: Annotation) -> Option<Self> {
        if !matches!(draft, Annotation::Text { .. }) {
            return None;
        }
        Some(Self {
            previewed: None,
            preview_texture: None,
            index: None,
            original: draft.clone(),
            draft,
            focus: true,
            composing: false,
            error: String::new(),
        })
    }
    fn preview(&self, document: &Document) -> Result<image::RgbaImage, EditError> {
        let draft = if matches!(&self.draft,Annotation::Text{text,..} if text.is_empty()) {
            None
        } else {
            Some(&self.draft)
        };
        match self.index {
            Some(index) => document.preview_replacement(index, draft),
            None => document.preview(draft),
        }
    }
    pub fn paint_preview(&mut self, ui: &egui::Ui, document: &Document) {
        if self.previewed.as_ref() != Some(&self.draft) {
            match self.preview(document) {
                Ok(image) => {
                    let pixels = egui::ColorImage::from_rgba_unmultiplied(
                        [image.width() as usize, image.height() as usize],
                        image.as_raw(),
                    );
                    if let Some(texture) = &mut self.preview_texture {
                        texture.set(pixels, egui::TextureOptions::NEAREST);
                    } else {
                        self.preview_texture = Some(ui.ctx().load_texture(
                            "text-draft-preview",
                            pixels,
                            egui::TextureOptions::NEAREST,
                        ));
                    }
                    self.previewed = Some(self.draft.clone());
                }
                Err(error) => self.error = error.to_string(),
            }
        }
        if let Some(texture) = &self.preview_texture {
            let available = ui.ctx().content_rect();
            let size = egui::vec2(texture.size()[0] as f32, texture.size()[1] as f32);
            let scale = (available.width() / size.x).min(available.height() / size.y);
            let rect = egui::Rect::from_center_size(available.center(), size * scale);
            ui.painter().image(
                texture.id(),
                rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
    }
    fn apply(&self, document: &mut Document) -> Result<(), EditError> {
        let Some(index) = self.index else {
            if matches!(&self.draft, Annotation::Text { text, .. } if text.is_empty()) {
                return Ok(());
            }
            return document.add(self.draft.clone());
        };
        if document.annotations().get(index) != Some(&self.original) {
            return Err(EditError::InvalidIndex);
        }
        if self.draft == self.original {
            return Ok(());
        }
        if matches!(&self.draft, Annotation::Text { text, .. } if text.is_empty()) {
            document.remove(index)
        } else {
            document.replace(index, self.draft.clone())
        }
    }
    #[allow(deprecated)] // Some integrations still emit Disabled without an empty preedit.
    fn protect_composition(&mut self, events: &[egui::Event]) -> bool {
        let mut protected = self.composing;
        for event in events {
            match event {
                egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => {
                    self.composing = !text.is_empty();
                    protected |= self.composing;
                }
                egui::Event::Ime(egui::ImeEvent::Commit(text)) => {
                    protected |= !text.is_empty();
                    self.composing = false;
                }
                egui::Event::Ime(egui::ImeEvent::Disabled) => self.composing = false,
                _ => {}
            }
        }
        protected
    }
    /// None keeps the dialog open; Some(true) applied, Some(false) cancelled.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        document: &mut Document,
        commit_shortcut: bool,
    ) -> Option<bool> {
        let protected = ctx.input(|input| self.protect_composition(&input.events));
        let commit_shortcut = commit_shortcut && !protected;
        if commit_shortcut {
            ctx.input_mut(|input| {
                input.consume_key(egui::Modifiers::CTRL, egui::Key::Enter);
            });
        }
        let before = self.draft.clone();
        let mut open = true;
        let mut cancel = !protected && ctx.input(|input| input.key_pressed(egui::Key::Escape));
        let mut apply = commit_shortcut;
        egui::Window::new(tr("RustEditor", "Edit text"))
            .default_pos(egui::pos2(20.0, 20.0))
            .default_width(440.0)
            .collapsible(false)
            .open(&mut open)
            .show(ctx, |ui| {
                if let Annotation::Text {
                    text,
                    font_size,
                    family,
                    format,
                    color,
                    ..
                } = &mut self.draft
                {
                    let response = ui.add(
                        egui::TextEdit::multiline(text)
                            .desired_rows(5)
                            .desired_width(f32::INFINITY)
                            .char_limit(1_000_000),
                    );
                    if self.focus {
                        response.request_focus();
                        self.focus = false;
                    }
                    ui.horizontal(|ui| {
                        ui.label(tr("RustEditor", "Font family"));
                        ui.text_edit_singleline(family);
                        ui.add(
                            egui::DragValue::new(font_size)
                                .range(1.0..=1000.0)
                                .prefix(tr("RustEditor", "Pixels: ")),
                        );
                        ui.color_edit_button_srgba_unmultiplied(color);
                    });
                    format_controls(ui, format);
                }
                ui.label(tr(
                    "RustEditor",
                    "Apply empty text to remove this annotation.",
                ));
                ui.horizontal(|ui| {
                    apply |= ui
                        .add_enabled(!protected, egui::Button::new(tr("RustEditor", "Apply")))
                        .clicked();
                    cancel |= ui.button(tr("ColorGrabWidget", "Cancel")).clicked();
                });
                if !self.error.is_empty() {
                    ui.colored_label(egui::Color32::RED, &self.error);
                }
            });
        if self.draft != before {
            ctx.request_repaint();
        }
        if cancel || !open {
            return Some(false);
        }
        if apply {
            match self.apply(document) {
                Ok(()) => return Some(true),
                Err(error) => self.error = error.to_string(),
            }
        }
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use flameshot_core::editor::Point;
    fn document() -> Document {
        let mut doc = Document::new(image::RgbaImage::new(100, 100), 100).unwrap();
        doc.add(Annotation::Text {
            position: Point { x: 10.0, y: 20.0 },
            text: "Original".into(),
            font_size: 12.0,
            family: "serif".into(),
            format: Default::default(),
            color: [255, 0, 0, 128],
        })
        .unwrap();
        doc
    }
    #[test]
    fn dismissing_preedit_protects_the_same_frame_but_not_the_next_escape() {
        let mut doc = document();
        let original = doc.annotations().to_vec();
        let mut edit = TextEdit::new(&doc, 0).unwrap();
        edit.composing = true;
        let ctx = egui::Context::default();
        let escape = egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![
                    egui::Event::Ime(egui::ImeEvent::Preedit {
                        text: String::new(),
                        active_range_chars: None,
                    }),
                    escape.clone(),
                ],
                ..Default::default()
            },
            |ui| {
                assert_eq!(edit.show(ui.ctx(), &mut doc, false), None);
            },
        );
        output.textures_delta.clear();
        assert!(!edit.composing);
        assert_eq!(doc.annotations(), original);
        let mut release = escape.clone();
        if let egui::Event::Key { pressed, .. } = &mut release {
            *pressed = false;
        }
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![release, escape],
                ..Default::default()
            },
            |ui| {
                assert_eq!(edit.show(ui.ctx(), &mut doc, false), Some(false));
            },
        );
        output.textures_delta.clear();
        assert_eq!(doc.annotations(), original);
    }
    #[test]
    fn ime_composition_does_not_cancel_or_apply_the_annotation() {
        let mut doc = document();
        let original = doc.annotations()[0].clone();
        let mut editor = TextEdit::new(&doc, 0).unwrap();
        if let Annotation::Text { text, .. } = &mut editor.draft {
            text.clear();
        }
        let ctx = egui::Context::default();
        let mut frame = |events, commit| {
            let mut result = None;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    result = editor.show(ui.ctx(), &mut doc, commit);
                },
            );
            output.textures_delta.clear();
            result
        };
        assert_eq!(frame(vec![], false), None);
        assert_eq!(frame(vec![], false), None);
        assert_eq!(
            frame(
                vec![egui::Event::Ime(egui::ImeEvent::Preedit {
                    text: "zhong".into(),
                    active_range_chars: None
                }),],
                true
            ),
            None
        );
        assert_eq!(
            frame(
                vec![egui::Event::Ime(egui::ImeEvent::Commit("中文".into()))],
                true
            ),
            None
        );
        assert_eq!(frame(vec![], true), Some(true));
        assert!(matches!(&doc.annotations()[0], Annotation::Text{text,..} if text == "中文"));
        assert!(doc.undo());
        assert_eq!(doc.annotations()[0], original);
    }
    #[test]
    fn draft_is_isolated_and_apply_is_one_undoable_replacement() {
        let mut doc = document();
        let original = doc.annotations()[0].clone();
        let mut edit = TextEdit::new(&doc, 0).unwrap();
        if let Annotation::Text {
            text,
            font_size,
            family,
            format,
            color,
            ..
        } = &mut edit.draft
        {
            *text = "Edited\nMultiline".into();
            *font_size = 24.0;
            *family = "sans-serif".into();
            *color = [0, 255, 0, 255];
            *format = TextStyle {
                bold: true,
                italic: true,
                underline: true,
                strikeout: true,
                alignment: TextAlignment::Right,
            };
        }
        assert_eq!(doc.annotations(), std::slice::from_ref(&original));
        edit.apply(&mut doc).unwrap();
        assert_eq!(doc.annotations().len(), 1);
        assert_eq!(doc.annotations()[0], edit.draft);
        assert!(doc.undo());
        assert_eq!(doc.annotations(), &[original]);
        assert!(doc.redo());
        assert_eq!(doc.annotations(), &[edit.draft]);
    }
    #[test]
    fn empty_new_text_commit_does_not_insert_a_newline_or_history_entry() {
        let mut doc = document();
        let mut draft = doc.annotations()[0].clone();
        if let Annotation::Text { text, .. } = &mut draft {
            text.clear();
        }
        let mut editor = TextEdit::create(draft).unwrap();
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::CTRL,
            }],
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            assert_eq!(editor.show(ui.ctx(), &mut doc, true), Some(true));
        });
        output.textures_delta.clear();
        assert_eq!(doc.annotations().len(), 1);
        assert!(doc.undo());
        assert!(doc.annotations().is_empty());
    }
    #[test]
    fn cancel_noop_empty_text_and_stale_index() {
        let mut doc = document();
        let edit = TextEdit::new(&doc, 0).unwrap();
        edit.apply(&mut doc).unwrap();
        assert!(doc.undo());
        assert!(doc.annotations().is_empty());
        assert!(!doc.undo());
        assert!(edit.apply(&mut doc).is_err());
        doc.redo();
        let mut edit = TextEdit::new(&doc, 0).unwrap();
        if let Annotation::Text { text, .. } = &mut edit.draft {
            text.clear();
        }
        drop(edit);
        assert_eq!(doc.annotations().len(), 1);
        let mut edit = TextEdit::new(&doc, 0).unwrap();
        if let Annotation::Text { text, .. } = &mut edit.draft {
            text.clear();
        }
        edit.apply(&mut doc).unwrap();
        assert!(doc.annotations().is_empty());
        doc.undo();
        assert_eq!(doc.annotations().len(), 1);
    }
}
