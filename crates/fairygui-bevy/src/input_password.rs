//! Masked geometry for an unchanged, real-value EditableText editor.
use super::*;
use bevy::text::{EditableText, FontCx, LayoutCx, LineHeight, TextEdit, TextLayoutInfo};
use bevy::ui::ComputedUiRenderTargetInfo;

#[cfg(test)]
#[path = "input_password_tests.rs"]
mod tests;

#[derive(Component, Debug)]
pub(crate) struct InputPasswordView(pub Entity);

/// This is a layout-only child: no Node, Text, GObjectRef or pointer target.
/// Bevy computes atlas glyphs for it, but only the owner's geometry is extracted.
#[derive(Component, Debug, Default)]
pub(crate) struct PasswordLayout {
    source: String,
    map: PasswordMap,
}

/// UTF-8 editor boundaries mapped to Laya's UTF-16 mask, with CRLF folded.
/// Endpoints never land inside a Rust scalar (including astral surrogate pairs).
#[derive(Debug)]
struct PasswordMap {
    text: String,
    boundaries: Vec<(usize, usize)>,
}
impl Default for PasswordMap {
    fn default() -> Self {
        Self::new("")
    }
}
impl PasswordMap {
    fn new(text: &str) -> Self {
        let mut boundaries = vec![(0, 0)];
        let mut display = 0;
        let mut chars = text.char_indices().peekable();
        while let Some((at, c)) = chars.next() {
            let mut end = at + c.len_utf8();
            if c == '\r' && chars.peek().is_some_and(|(_, c)| *c == '\n') {
                end = chars.next().unwrap().0 + 1;
            }
            display += c.len_utf16() * '●'.len_utf8();
            boundaries.push((end, display));
        }
        Self {
            text: "●".repeat(display / '●'.len_utf8()),
            boundaries,
        }
    }
    fn display(&self, index: usize) -> usize {
        let at = self.boundaries.partition_point(|&(raw, _)| raw <= index);
        self.boundaries[at.saturating_sub(1)].1
    }
    fn source(&self, index: usize) -> usize {
        let at = self.boundaries.partition_point(|&(_, mask)| mask < index);
        let after = self.boundaries[at.min(self.boundaries.len() - 1)];
        let before = self.boundaries[at.saturating_sub(1)];
        if index.saturating_sub(before.1) < after.1.saturating_sub(index) {
            before.0
        } else {
            after.0
        }
    }
}

pub(crate) fn sync_password_views(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    owners: Query<(
        Entity,
        &GObjectRef,
        Option<&EditableText>,
        Option<&InputPasswordView>,
    )>,
) {
    for (entity, owner, editor, view) in &owners {
        let password = matches!(runtime.store().object(owner.0).map(|o| o.control()),
            Some(ControlData::TextInput(data)) if data.password);
        if password && editor.is_some() {
            if view.is_none() {
                let editor = EditableText {
                    visible_lines: None,
                    ..default()
                };
                let child = commands
                    .spawn((
                        PasswordLayout::default(),
                        editor,
                        TextLayoutInfo::default(),
                        ComputedUiRenderTargetInfo::default(),
                        ChildOf(entity),
                    ))
                    .id();
                // TextInputPlugin globally requires Node/ComputedNode for every
                // EditableText. This editor must remain layout-only in full apps.
                commands
                    .entity(child)
                    .remove::<(Node, ComputedNode)>()
                    .insert(bevy::picking::Pickable::IGNORE);
                commands.entity(entity).insert(InputPasswordView(child));
            }
        } else if let Some(view) = view {
            commands.entity(view.0).try_despawn();
            commands.entity(entity).remove::<InputPasswordView>();
            // Force restoration of raw glyphs after publishing masked geometry.
            // PlainEditor set_text is deliberately avoided: keep selection/IME.
            if editor.is_some() {
                commands.queue(move |world: &mut World| {
                    if let Some(mut editor) = world.get_mut::<EditableText>(entity) {
                        let scale = editor.editor.get_scale();
                        editor.editor.set_scale(scale);
                    }
                });
            }
        }
    }
}

fn project_editor(
    source: &EditableText,
    target: &mut EditableText,
    map: &PasswordMap,
    fonts: &mut FontCx,
    layouts: &mut LayoutCx,
) {
    let compose = source
        .editor
        .raw_compose()
        .as_ref()
        .map(|r| map.display(r.start)..map.display(r.end));
    if target.editor.raw_text() != map.text || *target.editor.raw_compose() != compose {
        target.editor.set_text(&map.text);
    }
    let selection = source.editor.raw_selection();
    let start = map.display(selection.anchor().index());
    let end = map.display(selection.focus().index());
    let mut driver = target.editor.driver(fonts, &mut layouts.0);
    driver.refresh_layout();
    if driver.editor.raw_selection().anchor().index() != start
        || driver.editor.raw_selection().focus().index() != end
    {
        driver.select_byte_range(start, end);
    }
    if *driver.editor.raw_compose() != compose
        && let Some(range) = compose
    {
        driver.set_compose_byte_range(range.start, range.end);
    }
}

/// Pointer hit tests and scrolling operate in the mask's coordinate space.
/// The source editor still performs actual Unicode text mutations.
pub(super) fn apply_projected_edit(
    edit: &TextEdit,
    source: &mut EditableText,
    target: &mut EditableText,
    fonts: &mut FontCx,
    layouts: &mut LayoutCx,
    clipboard: &mut bevy::clipboard::Clipboard,
) -> bool {
    if !matches!(
        edit,
        TextEdit::MoveToPoint(_)
            | TextEdit::SelectWordAtPoint(_)
            | TextEdit::SelectLineAtPoint(_)
            | TextEdit::SelectedHardLineAtPoint(_)
            | TextEdit::ExtendSelectionToPoint(_)
            | TextEdit::ShiftClickExtension(_)
            | TextEdit::ScrollByLines(_)
            | TextEdit::ScrollTo(_)
            | TextEdit::ScrollBy(_)
            | TextEdit::Up(_)
            | TextEdit::Down(_)
            | TextEdit::LineStart(_)
            | TextEdit::LineEnd(_)
            | TextEdit::HardLineStart(_)
            | TextEdit::HardLineEnd(_)
    ) {
        return false;
    }
    let map = PasswordMap::new(source.editor.raw_text());
    project_editor(source, target, &map, fonts, layouts);
    let mut driver = target.editor.driver(fonts, &mut layouts.0);
    edit.clone().apply(
        &mut driver,
        &mut target.viewport,
        source.cursor_margin,
        clipboard,
        None,
        |_| true,
    );
    let selection = driver.editor.raw_selection();
    let start = map.source(selection.anchor().index());
    let end = map.source(selection.focus().index());
    source
        .editor
        .driver(fonts, &mut layouts.0)
        .select_byte_range(start, end);
    true
}

/// Runs after source styles, viewport synchronization and queued edits. It uses
/// the same Bevy font/atlas layout system as the owner without changing its value.
#[allow(clippy::type_complexity)]
pub(crate) fn prepare_password_layout(
    sources: Query<
        (
            &InputPasswordView,
            &EditableText,
            &TextFont,
            &TextLayout,
            &FontHinting,
            &LineHeight,
            Option<&ComputedUiRenderTargetInfo>,
        ),
        Without<PasswordLayout>,
    >,
    mut targets: Query<
        (
            &mut EditableText,
            &mut TextFont,
            &mut TextLayout,
            &mut FontHinting,
            &mut ComputedUiRenderTargetInfo,
            &mut LineHeight,
            &mut PasswordLayout,
        ),
        With<PasswordLayout>,
    >,
    fonts: Option<ResMut<FontCx>>,
    layouts: Option<ResMut<LayoutCx>>,
) {
    let (Some(mut fonts), Some(mut layouts)) = (fonts, layouts) else {
        return;
    };
    for (view, source, font, layout, hint, height, target_info) in &sources {
        let Ok((
            mut target,
            mut target_font,
            mut target_layout,
            mut target_hint,
            mut info,
            mut target_height,
            mut state,
        )) = targets.get_mut(view.0)
        else {
            continue;
        };
        let style_changed = target.is_added()
            || *target_font != *font
            || *target_height != *height
            || target_layout.justify != layout.justify
            || target_layout.linebreak != layout.linebreak
            || target.editor.get_scale() != source.editor.get_scale();
        if *target_font != *font {
            *target_font = font.clone();
        }
        if *target_height != *height {
            *target_height = *height;
        }
        if target_layout.justify != layout.justify || target_layout.linebreak != layout.linebreak {
            *target_layout = *layout;
        }
        if *target_hint != *hint {
            *target_hint = *hint;
        }
        if let Some(target_info) = target_info
            && *info != *target_info
        {
            *info = *target_info;
        }
        if style_changed {
            *target.editor.edit_styles() = source.editor.get_styles().clone();
            target.editor.set_scale(source.editor.get_scale());
        }
        if target.viewport.size != source.viewport.size {
            target.viewport.size = source.viewport.size;
            target.editor.set_width(Some(source.viewport.size.x));
        }
        if state.source != source.editor.raw_text() {
            state.source = source.editor.raw_text().to_owned();
            state.map = PasswordMap::new(&state.source);
        }
        let previous = target.editor.generation();
        project_editor(source, &mut target, &state.map, &mut fonts, &mut layouts);
        if previous != target.editor.generation() {
            let target = &mut *target;
            let mut driver = target.editor.driver(&mut fonts, &mut layouts.0);
            if let Some(cursor) = bevy::text::cursor_reveal_rect(&mut driver) {
                let layout = driver.layout();
                let size = Vec2::new(layout.full_width(), layout.height());
                target.viewport.reveal_caret(
                    cursor,
                    size,
                    source.cursor_margin,
                    layout
                        .lines()
                        .map(|line| bevy::text::TextLineYBounds::from_line(&line)),
                );
            }
        }
    }
}

/// Replace all draw geometry, including cursor, selection and IME underlines,
/// before extraction. Bevy's public editor buffer continues to hold the real value.
#[allow(clippy::type_complexity)]
pub(crate) fn publish_password_layout(
    mut sources: Query<
        (&InputPasswordView, &mut EditableText, &mut TextLayoutInfo),
        Without<PasswordLayout>,
    >,
    targets: Query<(&EditableText, &TextLayoutInfo), With<PasswordLayout>>,
) {
    for (view, mut source, mut info) in &mut sources {
        let Ok((target, target_info)) = targets.get(view.0) else {
            info.clear();
            continue;
        };
        let cursor = info.cursor.and_then(|(visible, rect)| {
            target.editor.cursor_geometry(rect.width()).map(|r| {
                (
                    visible,
                    Rect::new(r.x0 as f32, r.y0 as f32, r.x1 as f32, r.y1 as f32),
                )
            })
        });
        *info = target_info.clone();
        info.cursor = cursor;
        if source.viewport != target.viewport {
            source.viewport = target.viewport;
        }
    }
}
