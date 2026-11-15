//! Pointer selection must remain stable while the pointer is held still.
use super::*;
use bevy::text::{EditableText, TextEdit, TextLayoutInfo};

#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    step: u8,
    owner: Option<Entity>,
    input: Option<Entity>,
    cursor: Option<Vec2>,
    selection: Option<(usize, usize)>,
    offset: Vec2,
    case: u8,
    baseline: Vec<(usize, usize)>,
    scroll_offset: Vec2,
    scroll_focus: usize,
    autoscroll_case: u8,
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<State>().add_systems(
        PreUpdate,
        drive
            .after(InputSystems)
            .before(bevy::input_focus::InputFocusSystems::Dispatch)
            .before(UiSystems::Focus)
            .before(PickingSystems::ProcessInput),
    );
}

#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    demo: Res<Demo>,
    assets: Res<AssetServer>,
    gui: GuiQuery,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    mut editors: Query<&mut EditableText>,
    layouts: Query<&TextLayoutInfo>,
    focus: Res<GuiInputState>,
    mut windows: Query<(Entity, &mut Window)>,
    mut cameras: Query<&mut Camera, With<Camera2d>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    assert!(
        state.frame < 1600,
        "text selection timeout at step {}",
        state.step
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    let elapsed = state.frame - state.since;
    let viewport_origin = cameras
        .single()
        .ok()
        .and_then(Camera::physical_viewport_rect)
        .map_or(Vec2::ZERO, |rect| rect.min.as_vec2());
    let point2 = |entity, point: Vec2| {
        let (node, transform) = nodes.get(entity).unwrap();
        (transform.transform_point2(node.content_box().min + point) + viewport_origin)
            / window.scale_factor()
    };
    let point = |entity, x| point2(entity, Vec2::new(x, 24.));
    let mut next = false;
    match state.step {
        0 if state.frame > 30 => {
            let Some(menu) = demo.menu_owner.and_then(|owner| gui.view(owner)) else {
                return;
            };
            let mut display = *gui.display(menu).unwrap();
            display.visible = false;
            commands.entity(menu).insert(display);
            state.owner = Some(
                commands
                    .spawn(
                        FairyGuiRoot::new(assets.load("ui/Basics.fui"), "Demo_Text")
                            .with_position(Vec2::new(70., 80.)),
                    )
                    .id(),
            );
            next = true;
        }
        1 if elapsed > 12 && gui.view(state.owner.unwrap()).is_some() => {
            let owner = state.owner.unwrap();
            let input = gui.child_by_name(owner, "n22").unwrap();
            for &child in gui.get(owner).unwrap().children() {
                let mut display = *gui.display(child).unwrap();
                display.visible = child == input;
                commands.entity(child).insert(display);
            }
            let mut format = gui.text_format(input).unwrap().clone();
            format.font = None;
            format.font_size = 32;
            format.single_line = true;
            format.auto_size = 0;
            format.vertical_align = 1;
            let mut transform = *gui.transform(input).unwrap();
            transform.position = Vec2::new(30., 40.);
            transform.size = Vec2::new(600., 90.);
            commands.entity(input).insert((
                format,
                transform,
                GuiText::new("Hello 中文 selection sample 0123456789"),
                GuiTextInputSettings::default(),
            ));
            state.input = Some(input);
            next = true;
        }
        2 if elapsed > 12 => {
            let mut editor = editors.get_mut(state.input.unwrap()).unwrap();
            editor.queue_edit(TextEdit::TextStart(false));
            state.cursor = Some(point(
                state.input.unwrap(),
                if state.case == 1 { 190. } else { 8. },
            ));
            next = true;
        }
        3 if elapsed > 6 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        4 if elapsed > 4 => {
            assert_eq!(focus.focused_entity(), state.input);
            state.cursor = Some(point(
                state.input.unwrap(),
                if state.case == 1 { 8. } else { 190. },
            ));
            next = true;
        }
        5 if elapsed > 4 => {
            let editor = editors.get(state.input.unwrap()).unwrap();
            let selection = editor.editor.raw_selection();
            state.selection = Some((selection.anchor().index(), selection.focus().index()));
            if state.case < 3 {
                let selected = state.selection.unwrap();
                state.baseline.push(selected);
            } else {
                assert_eq!(
                    state.selection.unwrap(),
                    state.baseline[if state.case == 3 { 0 } else { 2 }],
                    "camera viewport origin changed the selected text"
                );
            }
            state.offset = editor.viewport.offset;
            assert_ne!(
                state.selection.unwrap().0,
                state.selection.unwrap().1,
                "drag must select text"
            );
            if state.case == 1 {
                assert!(
                    state.selection.unwrap().0 > state.selection.unwrap().1,
                    "reverse drag direction"
                );
            }
            assert!(
                !layouts
                    .get(state.input.unwrap())
                    .unwrap()
                    .selection_rects
                    .is_empty()
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(format!(
                    "target/fairygui-demo-text-selection-{}.png",
                    state.case
                )));
            next = true;
        }
        6 => {
            let entity = state.input.unwrap();
            let editor = editors.get(entity).unwrap();
            let selection = editor.editor.raw_selection();
            assert_eq!(
                Some((selection.anchor().index(), selection.focus().index())),
                state.selection,
                "stationary drag changed selection: frame={}, offset={:?}, initial_offset={:?}",
                state.frame,
                editor.viewport.offset,
                state.offset
            );
            assert!(
                !layouts.get(entity).unwrap().selection_rects.is_empty(),
                "selection paint disappeared"
            );
            if elapsed > 16 {
                mouse.release(MouseButton::Left);
                next = true;
            }
        }
        7 if elapsed > 8 => {
            let editor = editors.get(state.input.unwrap()).unwrap();
            let selection = editor.editor.raw_selection();
            assert_eq!(
                Some((selection.anchor().index(), selection.focus().index())),
                state.selection,
                "release must retain selection"
            );
            let mut transform = *gui.transform(state.input.unwrap()).unwrap();
            transform.size.x += 20.;
            commands.entity(state.input.unwrap()).insert(transform);
            next = true;
        }
        8 if elapsed > 8 => {
            let entity = state.input.unwrap();
            let editor = editors.get(entity).unwrap();
            let selection = editor.editor.raw_selection();
            assert_eq!(
                Some((selection.anchor().index(), selection.focus().index())),
                state.selection,
                "resize must retain selection"
            );
            assert!(
                !layouts.get(entity).unwrap().selection_rects.is_empty(),
                "resize erased selection paint"
            );
            if state.case < 4 {
                state.case += 1;
                commands.entity(entity).insert(GuiTextInputSettings {
                    password: matches!(state.case, 2 | 4),
                    ..default()
                });
                // Keep the physical field geometry comparable to the original
                // drag while moving the camera's viewport within the window.
                let mut transform = *gui.transform(entity).unwrap();
                transform.size.x = 600.;
                commands.entity(entity).insert(transform);
                if state.case == 3 {
                    cameras.single_mut().unwrap().viewport = Some(bevy::camera::Viewport {
                        physical_position: UVec2::new(140, 80),
                        physical_size: window.physical_size() - UVec2::new(140, 80),
                        ..default()
                    });
                }
                state.selection = None;
                state.step = 2;
                state.since = state.frame;
            } else {
                editors
                    .get_mut(entity)
                    .unwrap()
                    .queue_edit(TextEdit::SelectAll);
                next = true;
            }
        }
        9 if elapsed > 8 => {
            let entity = state.input.unwrap();
            let editor = editors.get(entity).unwrap();
            assert_eq!(
                editor.editor.selected_text().unwrap(),
                editor.editor.raw_text()
            );
            assert!(
                !layouts.get(entity).unwrap().selection_rects.is_empty(),
                "select all paint disappeared"
            );
            next = true;
        }
        10 if elapsed > 8 => {
            state.cursor = Some(point(state.input.unwrap(), 8.));
            next = true;
        }
        11 if elapsed > 6 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        12 if elapsed > 4 => {
            state.cursor = Some(point(state.input.unwrap(), 190.));
            next = true;
        }
        13 if elapsed > 4 => {
            assert!(
                !editors
                    .get(state.input.unwrap())
                    .unwrap()
                    .editor
                    .raw_selection()
                    .is_collapsed()
            );
            commands.queue(|world: &mut World| {
                use bevy::ecs::system::RunSystemOnce;
                world
                    .run_system_once(|mut input: GuiInput| input.blur())
                    .unwrap();
            });
            next = true;
        }
        14 if elapsed > 4 => {
            assert_eq!(focus.focused_entity(), None);
            assert!(
                editors
                    .get(state.input.unwrap())
                    .unwrap()
                    .editor
                    .raw_selection()
                    .is_collapsed(),
                "held mouse restored a selection after blur"
            );
            assert!(
                layouts
                    .get(state.input.unwrap())
                    .unwrap()
                    .selection_rects
                    .is_empty()
            );
            if elapsed > 16 {
                mouse.release(MouseButton::Left);
                let entity = state.input.unwrap();
                commands
                    .entity(entity)
                    .insert(GuiText::new("iW中文0123456789 ".repeat(40)));
                let mut transform = *gui.transform(entity).unwrap();
                transform.size = Vec2::new(200., 90.);
                commands.entity(entity).insert(transform);
                next = true;
            }
        }
        15 if elapsed > 10 => {
            editors
                .get_mut(state.input.unwrap())
                .unwrap()
                .queue_edit(TextEdit::TextStart(false));
            state.cursor = Some(point(state.input.unwrap(), 8.));
            next = true;
        }
        16 if elapsed > 8 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        17 if elapsed > 4 => {
            state.cursor = Some(if state.autoscroll_case == 2 {
                point2(state.input.unwrap(), Vec2::new(70., 200.))
            } else {
                point(state.input.unwrap(), 400.)
            });
            next = true;
        }
        18 if elapsed > 4 => {
            let editor = editors.get(state.input.unwrap()).unwrap();
            state.scroll_offset = editor.viewport.offset;
            state.scroll_focus = editor.editor.raw_selection().focus().index();
            next = true;
        }
        19 if elapsed > 30 => {
            let editor = editors.get(state.input.unwrap()).unwrap();
            assert!(
                if state.autoscroll_case == 2 {
                    editor.viewport.offset.y > state.scroll_offset.y + 1.
                } else {
                    editor.viewport.offset.x > state.scroll_offset.x + 1.
                },
                "outside stationary drag must scroll: before={:?}, after={:?}",
                state.scroll_offset,
                editor.viewport.offset
            );
            assert!(
                editor.editor.raw_selection().focus().index() > state.scroll_focus,
                "outside stationary drag must extend the selection"
            );
            state.cursor = Some(point(state.input.unwrap(), 100.));
            next = true;
        }
        20 if elapsed > 8 => {
            state.scroll_offset = editors.get(state.input.unwrap()).unwrap().viewport.offset;
            next = true;
        }
        21 if elapsed > 12 => {
            assert!(
                editors
                    .get(state.input.unwrap())
                    .unwrap()
                    .viewport
                    .offset
                    .abs_diff_eq(state.scroll_offset, 0.01),
                "reentry must stop automatic scrolling"
            );
            state.scroll_focus = editors
                .get(state.input.unwrap())
                .unwrap()
                .editor
                .raw_selection()
                .focus()
                .index();
            state.cursor = Some(if state.autoscroll_case == 2 {
                point2(state.input.unwrap(), Vec2::new(70., -150.))
            } else {
                point(state.input.unwrap(), -150.)
            });
            next = true;
        }
        22 if elapsed > 30 => {
            let editor = editors.get(state.input.unwrap()).unwrap();
            assert!(
                if state.autoscroll_case == 2 {
                    editor.viewport.offset.y < state.scroll_offset.y - 1.
                } else {
                    editor.viewport.offset.x < state.scroll_offset.x - 1.
                },
                "reverse drag must scroll back"
            );
            assert!(editor.editor.raw_selection().focus().index() < state.scroll_focus);
            state.scroll_offset = editor.viewport.offset;
            mouse.release(MouseButton::Left);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(format!(
                    "target/fairygui-demo-selection-autoscroll-{}.png",
                    state.autoscroll_case
                )));
            next = true;
        }
        23 if elapsed > 12 => {
            assert!(
                editors
                    .get(state.input.unwrap())
                    .unwrap()
                    .viewport
                    .offset
                    .abs_diff_eq(state.scroll_offset, 0.01),
                "release must stop automatic scrolling"
            );
            if state.autoscroll_case < 2 {
                state.autoscroll_case += 1;
                let entity = state.input.unwrap();
                commands
                    .entity(entity)
                    .insert(GuiTextInputSettings::default());
                let mut format = gui.text_format(entity).unwrap().clone();
                format.single_line = state.autoscroll_case != 2;
                commands.entity(entity).insert((
                    format,
                    GuiText::new(if state.autoscroll_case == 2 {
                        "line 中文 selection with wrapped words\n".repeat(40)
                    } else {
                        "iW中文0123456789 ".repeat(40)
                    }),
                ));
                state.step = 15;
                state.since = state.frame;
            } else {
                info!(
                    "demo smoke PASSED: ordinary/password/multiline drag autoscroll, reverse, reentry, release, shifted viewports, selection stability and blur"
                );
                exit.write(AppExit::Success);
                next = true;
            }
        }
        _ => {}
    }
    if next {
        state.step += 1;
        state.since = state.frame;
    }
    window.set_cursor_position(state.cursor);
    move_pointer(window_entity, state.cursor, &mut pointers);
}
