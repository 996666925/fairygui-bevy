//! Native canvas display -> plain input session -> canvas display.
use super::*;
use bevy::text::{EditableText, TextEdit, TextLayoutInfo};
use bevy::window::Ime;

#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    step: u8,
    case: u8,
    owner: Option<Entity>,
    input: Option<Entity>,
    cursor: Option<Vec2>,
    value: String,
    first_caret: Option<usize>,
    replaced: bool,
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

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn drive(
    mut state: ResMut<State>,
    demo: Res<Demo>,
    gui: GuiQuery,
    mut assets: ResMut<Assets<FuiAsset>>,
    nodes: Query<(&Node, &ComputedNode, &UiGlobalTransform)>,
    layouts: Query<&TextLayoutInfo>,
    canvas: Query<(
        Option<&fairygui_bevy::runtime::BitmapGlyphs>,
        Option<&Children>,
    )>,
    spans: Query<(), With<TextSpan>>,
    images: Query<&ImageNode>,
    textures: Res<Assets<Image>>,
    mut services: ParamSet<(
        GuiInput,
        GuiMut,
        Query<&mut EditableText>,
        Query<(Entity, &mut Window)>,
    )>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut ime: MessageWriter<Ime>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    assert!(
        state.frame < 750,
        "input presentation timeout at case {} step {}",
        state.case,
        state.step
    );
    let (window_entity, scale) = {
        let mut windows = services.p3();
        let Ok((entity, mut window)) = windows.single_mut() else {
            return;
        };
        window.focused = true;
        (entity, window.scale_factor())
    };
    let elapsed = state.frame - state.since;
    let rich = state.case == 1 || state.case == 2;
    let bitmap = state.case != 2;
    let mut next = false;
    match state.step {
        0 if state.frame > 25 => {
            let Some(menu) = demo.menu_owner.filter(|&owner| gui.view(owner).is_some()) else {
                return;
            };
            services.p1().get_mut(menu).unwrap().set_visible(false);
            let asset = assets.add(FuiAsset {
                package: fairygui_core::FuiPackage::parse(
                    "Basics",
                    include_bytes!("../../fairygui-core/tests/fixtures/BitmapText.fui"),
                )
                .unwrap(),
                res_key: "ui/Basics".into(),
            });
            state.owner = Some(
                commands
                    .spawn(FairyGuiRoot::new(asset, "Demo_Text").with_position(Vec2::new(70., 80.)))
                    .id(),
            );
            next = true;
        }
        1 if elapsed > 8 && gui.view(state.owner.unwrap()).is_some() => {
            let owner = state.owner.unwrap();
            let input = gui.child_by_name(owner, "n22").unwrap();
            let mut objects = services.p1();
            for &child in gui.get(owner).unwrap().children() {
                objects.get_mut(child).unwrap().set_visible(child == input);
            }
            let mut format = gui.text_format(input).unwrap().clone();
            format.font = bitmap.then(|| "ui://BitmapText/HitNumber".into());
            format.font_size = 32;
            format.ubb_enabled = rich;
            format.html_enabled = false;
            format.single_line = true;
            format.auto_size = 0;
            format.align = if state.case == 2 {
                1
            } else if state.case == 3 {
                2
            } else {
                0
            };
            format.vertical_align = format.align;
            format.color = Color::WHITE;
            let mut transform = *gui.transform(input).unwrap();
            transform.position = Vec2::new(40., 60.);
            transform.size = Vec2::new(650., 100.);
            state.value = if rich {
                "[color=#FF8800]12[/color]3".into()
            } else {
                "123".into()
            };
            commands.entity(input).insert((
                format,
                transform,
                GuiText::new(&state.value),
                GuiTextInputSettings {
                    password: state.case == 3,
                    ..default()
                },
            ));
            state.input = Some(input);
            next = true;
        }
        2 if elapsed > 8 => {
            let input = state.input.unwrap();
            if bitmap && state.case != 3 {
                let glyphs = canvas.get(input).unwrap().0.unwrap();
                assert!(!glyphs.0.is_empty());
                if glyphs.0.iter().any(|&part| {
                    !images
                        .get(part)
                        .is_ok_and(|image| textures.contains(&image.image))
                }) {
                    return;
                }
            }
            assert!(
                services.p2().get(input).is_err(),
                "canvas must not be a plain editor before focus"
            );
            let (style, node, transform) = nodes.get(input).unwrap();
            assert!(
                (node.size() - gui.transform(input).unwrap().size() * scale)
                    .abs()
                    .max_element()
                    <= 1.,
                "canvas renderer must remain in Bevy layout after replacing a plain editor"
            );
            println!(
                "input presentation {} canvas: model={:?} align={}/{} size={:?} style_padding={:?} padding={:?} center={:?}",
                state.case,
                gui.transform(input).unwrap(),
                gui.text_format(input).unwrap().align,
                gui.text_format(input).unwrap().vertical_align,
                node.size(),
                style.padding,
                node.padding,
                transform.transform_point2(Vec2::ZERO)
            );
            if let Some(glyphs) = canvas.get(input).unwrap().0
                && let Some(&glyph) = glyphs.0.first()
            {
                let (style, glyph_node, glyph_transform) = nodes.get(glyph).unwrap();
                println!(
                    "input presentation {} first glyph: local={:?}/{:?} size={:?} center={:?}",
                    state.case,
                    style.left,
                    style.top,
                    glyph_node.size(),
                    glyph_transform.transform_point2(Vec2::ZERO)
                );
            }
            state.cursor = Some(
                transform.transform_point2(node.content_box().min + Vec2::new(2., 18.)) / scale,
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(format!(
                    "target/fairygui-demo-input-presentation-{}-canvas.png",
                    state.case
                )));
            next = true;
        }
        3 if elapsed > 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        4 if elapsed > 5 => {
            let input = state.input.unwrap();
            assert_eq!(services.p0().focused_entity(), Some(input));
            let (glyphs, children) = canvas.get(input).unwrap();
            assert!(glyphs.is_none());
            assert!(
                !children
                    .is_some_and(|children| children.iter().any(|child| spans.contains(child)))
            );
            let caret = {
                let editors = services.p2();
                let editor = editors.get(input).unwrap();
                assert_eq!(editor.value().to_string(), state.value);
                editor.editor.raw_selection().focus().index()
            };
            if let Some(initial) = state.first_caret {
                assert_eq!(
                    caret, initial,
                    "initial click must match the stable editing viewport at the same window position"
                );
                state.first_caret = None;
                assert!(services.p0().select_all(input));
                next = true;
            } else {
                state.first_caret = Some(caret);
                mouse.release(MouseButton::Left);
                let (_, node, transform) = nodes.get(input).unwrap();
                let mut editors = services.p2();
                let mut editor = editors.get_mut(input).unwrap();
                let point = transform
                    .try_inverse()
                    .unwrap()
                    .transform_point2(state.cursor.unwrap() * scale)
                    - node.content_box().min
                    + editor.viewport.offset;
                editor.queue_edit(TextEdit::MoveToPoint(point));
                state.since = state.frame;
            }
        }
        5 if elapsed > 5 => {
            let input = state.input.unwrap();
            assert_eq!(
                services.p0().selected_text(input),
                Some(state.value.as_str())
            );
            assert!(!layouts.get(input).unwrap().selection_rects.is_empty());
            if state.case == 3 {
                assert_eq!(
                    layouts.get(input).unwrap().glyphs.len(),
                    state.value.encode_utf16().count()
                );
            }
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(format!(
                    "target/fairygui-demo-input-presentation-{}-editing.png",
                    state.case
                )));
            next = true;
        }
        6 if elapsed > 6 => {
            let input = state.input.unwrap();
            if !state.replaced {
                state.value = if rich {
                    "[color=#00FF88]67[/color]8".into()
                } else {
                    "678".into()
                };
                services
                    .p2()
                    .get_mut(input)
                    .unwrap()
                    .queue_edit(TextEdit::Insert(state.value.clone().into()));
                state.replaced = true;
                state.since = state.frame;
            } else {
                assert_eq!(
                    services.p2().get(input).unwrap().value().to_string(),
                    state.value
                );
                assert_eq!(gui.text(input), Some(state.value.as_str()));
                ime.write(Ime::Preedit {
                    window: window_entity,
                    value: "中".into(),
                    cursor: Some((0, 3)),
                });
                state.replaced = false;
                next = true;
            }
        }
        7 if elapsed > 5 => {
            assert!(
                services
                    .p2()
                    .get(state.input.unwrap())
                    .unwrap()
                    .is_composing()
            );
            ime.write(Ime::Commit {
                window: window_entity,
                value: "9".into(),
            });
            state.value.push('9');
            next = true;
        }
        8 if elapsed > 6 => {
            let input = state.input.unwrap();
            assert_eq!(
                services.p2().get(input).unwrap().value().to_string(),
                state.value
            );
            assert_eq!(gui.text(input), Some(state.value.as_str()));
            services.p0().blur();
            next = true;
        }
        9 if elapsed > 8 => {
            let input = state.input.unwrap();
            assert!(services.p2().get(input).is_err());
            let (glyphs, children) = canvas.get(input).unwrap();
            assert!(if bitmap {
                glyphs.is_some()
            } else {
                children.is_some_and(|children| children.iter().any(|child| spans.contains(child)))
            });
            assert_eq!(gui.child_by_name(state.owner.unwrap(), "n22"), Some(input));
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(format!(
                    "target/fairygui-demo-input-presentation-{}-restored.png",
                    state.case
                )));
            next = true;
        }
        10 if elapsed > 8 => {
            if state.case < 3 {
                state.case += 1;
                state.step = 1;
                state.since = state.frame;
            } else {
                commands.entity(state.owner.unwrap()).despawn();
                next = true;
            }
        }
        11 if elapsed > 8 => {
            assert!(gui.view(state.owner.unwrap()).is_none());
            info!(
                "demo smoke PASSED: bitmap, bitmap rich, raster rich and bitmap password canvas/input switching, first-click caret, selection paint, text/IME commits, retained entities and disposal"
            );
            exit.write(AppExit::Success);
        }
        _ => {}
    }
    if next {
        state.step += 1;
        state.since = state.frame;
    }
    if let Ok((_, mut window)) = services.p3().single_mut() {
        window.set_cursor_position(state.cursor);
    }
    move_pointer(window_entity, state.cursor, &mut pointers);
}
