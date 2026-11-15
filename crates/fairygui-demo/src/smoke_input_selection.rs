//! Public GUI caret/selection APIs against actual editable draw geometry.
use super::*;
use bevy::text::{EditableText, TextLayoutInfo};

#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    step: u8,
    case: u8,
    owner: Option<Entity>,
    input: Option<Entity>,
    value: String,
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<State>().add_systems(
        Update,
        (replace_ecs_text, drive)
            .chain()
            .in_set(GuiSystems::Respond),
    );
}

fn replace_ecs_text(state: Res<State>, mut texts: Query<&mut GuiText>) {
    // Apply an ordinary ECS edit immediately before the public selection call,
    // without allowing the scheduled GUI property synchronization in between.
    if state.step == 2
        && state.frame - state.since >= 8
        && let Some(input) = state.input
    {
        texts.get_mut(input).unwrap().set(&state.value);
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn drive(
    mut state: ResMut<State>,
    demo: Res<Demo>,
    assets: Res<AssetServer>,
    gui: GuiQuery,
    layouts: Query<&TextLayoutInfo>,
    mut services: ParamSet<(GuiInput, GuiMut, Query<&EditableText>, Query<&mut Window>)>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    assert!(
        state.frame < 900,
        "input selection timeout at step {}",
        state.step
    );
    for mut window in &mut services.p3() {
        window.focused = true;
    }
    let elapsed = state.frame - state.since;
    let mut next = false;
    match state.step {
        0 if state.frame > 30 => {
            let Some(menu) = demo.menu_owner.filter(|&owner| gui.view(owner).is_some()) else {
                return;
            };
            services.p1().get_mut(menu).unwrap().set_visible(false);
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
        1 if elapsed > 8 && gui.view(state.owner.unwrap()).is_some() => {
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
            format.single_line = state.case != 2;
            format.auto_size = 0;
            format.vertical_align = 0;
            let mut transform = *gui.transform(input).unwrap();
            transform.position = Vec2::new(30., 40.);
            transform.size = Vec2::new(300., 120.);
            state.value = if state.case == 2 {
                format!(
                    "A中😀Z\n{}",
                    "Line 中文 selection with several words\n".repeat(40)
                )
            } else {
                "A中😀Z ".repeat(60)
            };
            commands.entity(input).insert((
                format,
                transform,
                GuiText::new("old placeholder"),
                GuiTextInputSettings {
                    password: state.case == 1,
                    ..default()
                },
            ));
            state.input = Some(input);
            next = true;
        }
        2 if elapsed > 8 => {
            let input = state.input.unwrap();
            let mut service = services.p0();
            assert!(service.set_selection(input, GuiTextSelection::new(4, 1)));
            assert_eq!(service.selection(input), Some(GuiTextSelection::new(4, 1)));
            assert_eq!(service.selected_text(input), Some("中😀"));
            next = true;
        }
        3 if elapsed > 6 => {
            let input = state.input.unwrap();
            assert!(!layouts.get(input).unwrap().selection_rects.is_empty());
            assert_eq!(
                services.p2().get(input).unwrap().value().to_string(),
                state.value
            );
            let mut service = services.p0();
            assert_eq!(service.selected_text(input), Some("中😀"));
            assert!(service.set_caret_position(input, usize::MAX));
            assert_eq!(
                service.selection(input),
                Some(GuiTextSelection::caret(state.value.encode_utf16().count()))
            );
            next = true;
        }
        4 if elapsed > 6 => {
            let input = state.input.unwrap();
            let offset = services.p2().get(input).unwrap().viewport.offset;
            assert!(
                if state.case == 2 {
                    offset.y > 1.
                } else {
                    offset.x > 1.
                },
                "caret did not scroll into view, case={}: {offset:?}",
                state.case
            );
            assert!(services.p0().select_all(input));
            next = true;
        }
        5 if elapsed > 6 => {
            let input = state.input.unwrap();
            assert!(!layouts.get(input).unwrap().selection_rects.is_empty());
            assert_eq!(
                services.p0().selection(input).unwrap().range(),
                0..state.value.encode_utf16().count()
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(format!(
                    "target/fairygui-demo-input-selection-{}.png",
                    state.case
                )));
            next = true;
        }
        6 if elapsed > 6 => {
            // Let the selected frame reach the screenshot before collapsing it.
            assert!(services.p0().set_caret_position(state.input.unwrap(), 0));
            next = true;
        }
        7 if elapsed > 6 => {
            let input = state.input.unwrap();
            let offset = services.p2().get(input).unwrap().viewport.offset;
            assert!(
                offset.abs().max_element() <= 1.,
                "start caret did not scroll back: {offset:?}"
            );
            let mut service = services.p0();
            service.blur();
            assert!(service.set_selection(input, GuiTextSelection::new(4, 1)));
            next = true;
        }
        8 if elapsed > 6 => {
            let input = state.input.unwrap();
            assert_eq!(services.p0().selected_text(input), Some("中😀"));
            assert!(!layouts.get(input).unwrap().selection_rects.is_empty());
            assert_eq!(
                services.p2().get(input).unwrap().value().to_string(),
                state.value
            );
            services.p0().blur();
            if state.case < 2 {
                state.case += 1;
                state.step = 1;
                state.since = state.frame;
            } else {
                next = true;
            }
        }
        9 if elapsed > 20 => {
            info!(
                "demo smoke PASSED: same-frame ECS text edits, public UTF-16 selection, reversed ranges, selected real password text, caret reveal, select all, refocus and ordinary/password/multiline geometry"
            );
            exit.write(AppExit::Success);
            next = true;
        }
        _ => {}
    }
    if next {
        state.step += 1;
        state.since = state.frame;
    }
}
