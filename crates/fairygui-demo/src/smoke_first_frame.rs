//! Assert positions after layout on the first frame each view can be rendered.
use super::*;
use std::collections::HashSet;

#[derive(Resource, Default)]
struct State {
    frame: u32,
    step: u8,
    since: u32,
    roots: HashSet<Entity>,
    titles: HashSet<Entity>,
    cursor: Option<Vec2>,
    page: bool,
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<State>()
        .add_systems(Last, inspect)
        .add_systems(
            PreUpdate,
            drive
                .after(InputSystems)
                .before(UiSystems::Focus)
                .before(PickingSystems::ProcessInput),
        );
}
fn near(actual: Vec2, expected: Vec2) {
    assert!(
        actual.distance(expected) < 0.1,
        "first-frame position {actual:?}, expected {expected:?}"
    );
}
#[allow(clippy::too_many_arguments)]
fn inspect(
    mut state: ResMut<State>,
    demo: Res<Demo>,
    gui: GuiQuery,
    roots: Query<(Entity, &FairyGuiRoot, &GuiRootView)>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    texts: Query<(&bevy::text::TextLayoutInfo, &TextLayout)>,
    mut commands: Commands,
) {
    let Ok(window) = windows.single() else { return };
    let viewport = Vec2::new(window.width(), window.height());
    for (owner, source, view) in &roots {
        if !state.roots.insert(owner) {
            continue;
        }
        let transform = gui.transform(owner).unwrap();
        let expected = if source.item_name == "CloseButton" {
            viewport - transform.size() - Vec2::splat(10.)
        } else if source.item_name.starts_with("Demo_") {
            let parent = demo.open.as_ref().unwrap().0;
            let parent_position = gui.transform(parent).unwrap().position();
            let container = gui.child_by_name(parent, "container").unwrap();
            state.page = true;
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-first-frame-page.png"));
            parent_position + Vec2::new(0., gui.transform(container).unwrap().position().y)
        } else {
            (viewport - transform.size()) / 2.
        };
        near(transform.position(), expected);
        let (computed, global) = nodes.get(view.0).unwrap();
        let physical_expected =
            (expected + transform.size() / 2.) / computed.inverse_scale_factor();
        let physical_actual = global.transform_point2(Vec2::ZERO);
        assert!(
            physical_actual.distance(physical_expected) <= 1.,
            "first-frame render position {physical_actual:?}, expected {physical_expected:?} (physical pixels)"
        );
    }
    let candidates = [
        demo.menu_owner
            .and_then(|root| gui.child_by_path(root, "n1/title")),
        demo.open
            .as_ref()
            .and_then(|(root, _, _)| gui.child_by_path(*root, "btn_Button/title")),
    ];
    for title in candidates.into_iter().flatten() {
        let Ok((info, layout)) = texts.get(title) else {
            continue;
        };
        let Ok((computed, _)) = nodes.get(title) else {
            continue;
        };
        if info.size.y <= 0. || computed.size().min_element() <= 0. {
            continue;
        }
        let size = gui.transform(title).unwrap().size();
        let available = (size - info.size / info.scale_factor).max(Vec2::ZERO);
        let offset = Vec2::new(
            if layout.linebreak == bevy::text::LineBreak::NoWrap {
                available.x / 2.
            } else {
                0.
            },
            available.y / 2.,
        );
        near(
            computed.padding.min_inset * computed.inverse_scale_factor(),
            offset,
        );
        if state.titles.insert(title) && demo.open.is_none() {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-first-frame-menu.png"));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    demo: Res<Demo>,
    gui: GuiQuery,
    mut windows: Query<(Entity, &mut Window)>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut exit: MessageWriter<AppExit>,
    time: Res<Time<bevy::time::Real>>,
) {
    state.frame += 1;
    assert!(
        time.elapsed_secs_f64() < 45.,
        "first-frame smoke timed out at step {}",
        state.step
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    let elapsed = state.frame - state.since;
    let center = |entity| {
        gui.local_to_global(entity, gui.transform(entity).unwrap().size() / 2.)
            .unwrap()
    };
    let mut next = false;
    match state.step {
        0 if !state.titles.is_empty() => {
            state.cursor = Some(center(
                gui.child_by_name(demo.menu_owner.unwrap(), "n1").unwrap(),
            ));
            next = true;
        }
        1 | 5 | 8 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        2 | 6 | 9 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        3 if elapsed >= 6 => {
            let Some((owner, _, name)) = demo.open.as_ref() else {
                return;
            };
            assert_eq!(name, "Basics");
            if !state.roots.contains(owner) {
                return;
            }
            let button = gui.child_by_name(*owner, "btn_Button").unwrap();
            let title = gui.child_by_name(button, "title").unwrap();
            if !state.titles.contains(&title) {
                return;
            }
            state.cursor = Some(center(button));
            next = true;
        }
        4 if elapsed >= 4 => {
            next = true;
        }
        7 if elapsed >= 12 => {
            if !state.page {
                return;
            }
            assert!(state.roots.len() >= 4 && state.titles.len() >= 2);
            state.cursor = Some(center(gui.view(demo.close_owner.unwrap()).unwrap()));
            next = true;
        }
        10 if elapsed >= 8 && demo.open.is_none() => {
            println!(
                "demo smoke PASSED: first-rendered-frame root/menu/close-button/page placement and fresh text alignment; actual Basics and Button page navigation without settling frames"
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
    window.set_cursor_position(state.cursor);
    move_pointer(window_entity, state.cursor, &mut pointers);
}
