// SPDX-License-Identifier: GPL-3.0-or-later
//! Stable legacy tool IDs in CaptureToolButton priority order.
pub const CATALOG_CONTEXT: &str = "RustConfig";
pub fn translated_name(source: &str) -> &str {
    crate::i18n::tr(CATALOG_CONTEXT, source)
}
pub const CATALOG: &[(i32, &str)] = &[
    (0, "Pencil"),
    (1, "Line"),
    (2, "Arrow"),
    (3, "Outline rectangle"),
    (4, "Filled rectangle"),
    (5, "Ellipse"),
    (6, "Marker"),
    (18, "Text"),
    (15, "Pixelate"),
    (22, "Invert"),
    (19, "Circle counter"),
    (8, "Move selection"),
    (9, "Undo"),
    (16, "Redo"),
    (10, "Copy"),
    (11, "Save"),
    (13, "Upload"),
    (23, "Accept"),
    (14, "Open with"),
    (12, "Cancel"),
    (17, "Pin"),
    (20, "Increase size"),
    (21, "Decrease size"),
];
pub fn valid(id: i32) -> bool {
    CATALOG.iter().any(|(known, _)| *known == id)
}
pub fn available(id: i32) -> bool {
    id != 13 || cfg!(feature = "imgur")
}
pub fn priority(id: i32) -> usize {
    CATALOG
        .iter()
        .position(|(known, _)| *known == id)
        .unwrap_or(usize::MAX)
}
pub fn normalize(mut values: Vec<i32>) -> Vec<i32> {
    values.retain(|id| valid(*id));
    values.sort_by_key(|id| priority(*id));
    values.dedup();
    values
}
pub fn defaults() -> Vec<i32> {
    CATALOG
        .iter()
        .map(|(id, _)| *id)
        .filter(|id| available(*id) && !matches!(id, 20 | 21))
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_order_is_independent_of_storage_order_and_defaults_exclude_size_buttons() {
        assert_eq!(normalize(vec![23, 7, 2, 0, 2, 24, 18]), vec![0, 2, 18, 23]);
        assert!(!defaults().contains(&20));
        assert!(!defaults().contains(&21));
        assert_eq!(defaults().contains(&13), cfg!(feature = "imgur"));
    }
}
