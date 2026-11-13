//! Real picking, hover delay, edge placement, retained reuse and click-through.
use super::*;

#[derive(Resource, Default)]
struct State {
    frame: u32,
    step: u8,
    since: u32,
    entered: f64,
    cursor: Option<Vec2>,
    button: Option<Entity>,
    pane: Option<Entity>,
    edge_captured: bool,
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<State>()
        .add_systems(Startup, configure)
        .add_systems(
            PreUpdate,
            drive
                .after(InputSystems)
                .before(UiSystems::Focus)
                .before(PickingSystems::ProcessInput),
        );
}
fn configure(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(GuiTooltipSettings::new(
        assets.load("ui/Basics.fui"),
        "Component3",
    ));
}
#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    demo: Res<Demo>,
    gui: GuiQuery,
    tooltips: Res<GuiTooltipState>,
    input: Res<GuiInputState>,
    time: Res<Time>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    existing: Query<()>,
    mut windows: Query<(Entity, &mut Window), With<bevy::window::PrimaryWindow>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
    mut gui_edit: GuiMut,
) {
    state.frame += 1;
    assert!(
        state.frame < 1800,
        "tooltip smoke timed out at step {}: {tooltips:?}",
        state.step
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    let center =
        |entity| nodes.get(entity).unwrap().1.transform_point2(Vec2::ZERO) / window.scale_factor();
    let elapsed = state.frame - state.since;
    let mut next = false;
    match state.step {
        0 if state.frame > 30 && window.visible => {
            if let Some(button) = demo
                .menu_owner
                .and_then(|root| gui.child_by_name(root, "n1"))
                && nodes.get(button).is_ok_and(|(node, _)| node.size().x > 0.)
            {
                state.button = Some(button);
                gui_edit.edit(button, |object: &mut GuiObjectMut| {
                    object.set_tooltips(Some("Hover for details".into()))
                });
                state.cursor = Some(center(button));
                next = true;
            }
        }
        1 if input.hovered() == state.button => {
            assert!(!tooltips.is_showing());
            state.entered = time.elapsed_secs_f64();
            next = true;
        }
        2 if tooltips.is_showing() && elapsed > 12 => {
            assert!(time.elapsed_secs_f64() - state.entered >= 0.075);
            assert_eq!(tooltips.source(), state.button);
            let pane = tooltips.view().unwrap();
            assert_eq!(gui.text(pane), Some("Hover for details"));
            assert_eq!(input.hovered(), state.button);
            assert!(
                gui.transform(pane)
                    .unwrap()
                    .position()
                    .distance(state.cursor.unwrap() + Vec2::new(10., 20.))
                    < 0.1
            );
            if state.pane.is_none() {
                state.pane = Some(pane);
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk("target/fairygui-demo-tooltip-hover.png"));
                state.since = state.frame;
            } else {
                state.cursor = Some(Vec2::splat(5.));
                next = true;
            }
        }
        3 if elapsed > 5 => {
            assert!(!tooltips.is_showing());
            commands.write_message(GuiTooltipCommand::Show {
                text: "Near the window edge".into(),
                position: Some(Vec2::new(window.width() - 5., window.height() - 5.)),
            });
            next = true;
        }
        4 if elapsed > 5 => {
            assert!(tooltips.is_showing());
            assert_eq!(tooltips.view(), state.pane);
            let pane = gui.transform(state.pane.unwrap()).unwrap();
            assert!(pane.position().min_element() >= 0.);
            assert!(
                (pane.position() + pane.size())
                    .cmple(Vec2::new(window.width(), window.height()))
                    .all()
            );
            if !state.edge_captured {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk("target/fairygui-demo-tooltip-edge.png"));
                state.edge_captured = true;
                state.since = state.frame;
            } else {
                commands.write_message(GuiTooltipCommand::Hide);
                state.cursor = Some(center(state.button.unwrap()));
                next = true;
            }
        }
        5 if tooltips.is_showing() && tooltips.source() == state.button => {
            assert_eq!(tooltips.view(), state.pane);
            // Deliberately cover the button to verify picking passes through.
            commands.write_message(GuiTooltipCommand::Show {
                text: "Click through this tooltip".into(),
                position: Some(state.cursor.unwrap() - Vec2::splat(10.)),
            });
            next = true;
        }
        6 if elapsed > 6 => {
            assert!(tooltips.is_showing());
            assert_eq!(input.hovered(), state.button);
            mouse.press(MouseButton::Left);
            next = true;
        }
        7 if elapsed > 3 => {
            assert!(!tooltips.is_showing());
            mouse.release(MouseButton::Left);
            next = true;
        }
        8 if elapsed > 15 => {
            assert!(
                demo.open.is_some(),
                "the button under the tooltip must receive the click"
            );
            commands.write_message(GuiTooltipCommand::Dispose);
            next = true;
        }
        9 if elapsed > 5 => {
            assert!(!tooltips.is_requested() && !tooltips.is_showing());
            assert!(!existing.contains(state.pane.unwrap()));
            info!(
                "demo smoke PASSED: real tooltip hover delay, text, pointer offset, leave hide, native edge placement, retained reuse, input pass-through, click hide and disposal"
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
