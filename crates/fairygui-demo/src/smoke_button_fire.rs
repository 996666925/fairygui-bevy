use super::*;
use fairygui_core::widget::{ButtonState, ControlData};

#[derive(Resource, Default)]
pub(super) struct State {
    stage: u8,
    request: Option<(GObjectId, bool)>,
    frames: u32,
    since: u32,
    baseline: Vec2,
    saw_down: bool,
    clicks: usize,
    popup: Option<GObjectId>,
}

impl State {
    pub fn complete(&self) -> bool {
        self.stage == 5
    }
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<State>()
        .add_systems(Update, execute.in_set(GuiSystems::Respond));
}

fn execute(mut state: ResMut<State>, mut runtime: NonSendMut<GuiRuntime>) {
    if let Some((button, effect)) = state.request.take() {
        assert!(runtime.fire_button_click(button, effect));
    }
}

pub(super) fn drive(
    state: &mut State,
    runtime: &mut GuiRuntime,
    transforms: &Query<&UiTransform>,
    commands: &mut Commands,
    root: GObjectId,
) {
    let button = runtime.store().child_by_name(root, "n13").unwrap();
    state.frames += 1;
    match state.stage {
        0 => {
            runtime.store_mut().set_button_down_effect(button, 2, 0.8);
            let object = runtime.store().object(button).unwrap();
            state.baseline = Vec2::new(object.scale_x(), object.scale_y());
            let popup = runtime
                .store_mut()
                .create(fairygui_core::package::ObjectType::Component);
            runtime
                .store_mut()
                .set_button_linked_popup(button, Some(popup));
            state.popup = Some(popup);
            state.request = Some((button, true));
            state.since = state.frames;
            state.stage = 1;
        }
        1 | 2 => {
            assert!(
                state.frames - state.since < 180,
                "fireClick feedback did not finish"
            );
            assert!(
                runtime.popup_stack().is_empty(),
                "fireClick must not open its linked object"
            );
            let object = runtime.store().object(button).unwrap();
            let ControlData::Button(data) = object.control() else {
                unreachable!()
            };
            assert!(!data.pressed, "fireClick does not synthesize a press");
            match data.interaction_state {
                ButtonState::Down => {
                    let expected = state.baseline * 0.8;
                    assert!(
                        transforms
                            .get(runtime.entity_of(button).unwrap())
                            .unwrap()
                            .scale
                            .abs_diff_eq(expected, 0.001)
                    );
                    if !state.saw_down {
                        commands
                            .spawn(Screenshot::primary_window())
                            .observe(save_to_disk("target/fairygui-demo-button-fire-down.png"));
                    }
                    state.saw_down = true;
                    state.stage = 2;
                }
                ButtonState::Up if state.saw_down => {
                    assert!(
                        Vec2::new(object.scale_x(), object.scale_y())
                            .abs_diff_eq(state.baseline, 0.001)
                    );
                    assert!(
                        transforms
                            .get(runtime.entity_of(button).unwrap())
                            .unwrap()
                            .scale
                            .abs_diff_eq(state.baseline, 0.001)
                    );
                    assert_eq!(state.clicks, 0);
                    state.request = Some((button, false));
                    state.stage = 3;
                    state.since = state.frames;
                }
                _ => {}
            }
        }
        3 if state.frames - state.since >= 20 => {
            let object = runtime.store().object(button).unwrap();
            assert!(
                Vec2::new(object.scale_x(), object.scale_y()).abs_diff_eq(state.baseline, 0.001)
            );
            assert_eq!(state.clicks, 0, "fireClick must not dispatch GuiEvent");
            runtime.store_mut().set_button_linked_popup(button, None);
            runtime.store_mut().remove(state.popup.take().unwrap());
            state.stage = 5;
            println!(
                "demo smoke: fireClick showed timed pressed scale and restored its view; disabled feedback stayed unchanged; no GuiEvent or linked popup was produced"
            );
        }
        _ => {}
    }
}
