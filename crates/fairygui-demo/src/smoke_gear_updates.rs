use super::*;
use fairygui_core::{color::Color as GuiColor, widget::ControlData};

const BLUE: GuiColor = GuiColor::new(0.2, 0.6, 0.9, 1.0);
const GREEN: GuiColor = GuiColor::new(0.2, 0.85, 0.3, 1.0);
const ORANGE: GuiColor = GuiColor::new(0.95, 0.4, 0.15, 1.0);

#[derive(Default)]
pub(super) struct State {
    stage: u8,
    since: u32,
    sample: Option<usize>,
    ids: Vec<GObjectId>,
    views: Vec<Entity>,
    original_font: i32,
}
impl State {
    pub fn complete(&self) -> bool {
        self.stage == 12
    }
}

fn root(state: &State, runtime: &GuiRuntime) -> GObjectId {
    runtime.root_object(state.sample.unwrap()).unwrap()
}
fn set_page(state: &State, runtime: &mut GuiRuntime, page: i32) {
    let root = root(state, runtime);
    runtime
        .store_mut()
        .set_controller_page_index(root, "c1", page);
}
fn ready(state: &State, frames: u32, runtime: &GuiRuntime) -> bool {
    assert!(frames - state.since < 900, "gear roundtrip did not settle");
    frames - state.since >= 12 && !runtime.store().has_gear_tweens()
}
fn check(
    state: &State,
    runtime: &GuiRuntime,
    null_page: bool,
    nodes: &Query<&Node>,
    fonts: &Query<&TextFont>,
) {
    assert!(
        state
            .ids
            .iter()
            .zip(&state.views)
            .all(|(&id, &view)| runtime.entity_of(id) == Some(view))
    );
    let graph = runtime.store().object(state.ids[0]).unwrap();
    let expected = if null_page {
        (232.0, 310.0)
    } else {
        (210.0, 290.0)
    };
    assert_eq!((graph.x(), graph.y()), expected);
    let node = nodes.get(state.views[0]).unwrap();
    assert_eq!(
        (node.left, node.top),
        (Val::Px(expected.0), Val::Px(expected.1))
    );
    if !null_page {
        assert_eq!((node.width, node.height), (Val::Px(130.0), Val::Px(70.0)));
        let sprite = runtime.store().object(state.ids[1]).unwrap();
        assert!((sprite.alpha() - 0.55).abs() < 0.001 && (sprite.scale_x() - 1.1).abs() < 0.001);
        let ControlData::Text(text) = runtime.store().object(state.ids[3]).unwrap().control()
        else {
            panic!("text")
        };
        assert_eq!(text.color, ORANGE);
        assert_eq!(
            runtime
                .store()
                .object(state.ids[5])
                .unwrap()
                .control()
                .as_text_field()
                .unwrap()
                .font_size,
            state.original_font,
            "native font setter does not capture GearFontSize"
        );
        assert_eq!(
            fonts.get(state.views[5]).unwrap().font_size,
            bevy::text::FontSize::Px(state.original_font as f32)
        );
    }
    let ControlData::Image(image) = runtime.store().object(state.ids[2]).unwrap().control() else {
        panic!("image")
    };
    assert_eq!(image.color, Some(if null_page { GREEN } else { BLUE }));
    assert_eq!(
        runtime.store().animation_state(state.ids[4]),
        Some((false, 0))
    );
}

#[allow(clippy::too_many_arguments)]
pub(super) fn drive(
    state: &mut State,
    frames: u32,
    runtime: &mut GuiRuntime,
    assets: &mut Assets<FuiAsset>,
    nodes: &Query<&Node>,
    fonts: &Query<&TextFont>,
    computed: &Query<(&ComputedNode, &UiGlobalTransform)>,
    commands: &mut Commands,
    original: GObjectId,
) {
    match state.stage {
        0 => {
            let package = fairygui_core::FuiPackage::parse(
                "Basics",
                include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui"),
            )
            .unwrap();
            let handle = assets.add(FuiAsset {
                package: package.clone(),
                res_key: "ui/Basics".into(),
            });
            let object = runtime.store().object(original).unwrap();
            let position = Vec2::new(object.x(), object.y());
            let index = runtime
                .create_root(handle, &package, "Demo_Controller")
                .unwrap();
            let root = runtime.root_object(index).unwrap();
            runtime.store_mut().set_xy(root, position.x, position.y);
            runtime.store_mut().set_visible(original, false);
            state.ids = ["n3", "n7", "n9", "n6", "n10", "n17"]
                .map(|name| runtime.store().child_by_name(root, name).unwrap())
                .to_vec();
            state.original_font = runtime
                .store()
                .object(state.ids[5])
                .unwrap()
                .control()
                .as_text_field()
                .unwrap()
                .font_size;
            runtime.store_mut().set_xy(state.ids[0], 210.0, 290.0);
            runtime.store_mut().set_size(state.ids[0], 130.0, 70.0);
            runtime.store_mut().set_scale(state.ids[1], 1.1, 0.9);
            runtime.store_mut().set_alpha(state.ids[1], 0.55);
            runtime.store_mut().set_rotation(state.ids[1], 12.0);
            runtime
                .store_mut()
                .set_control_color(state.ids[2], BLUE, None);
            runtime
                .store_mut()
                .set_control_color(state.ids[3], ORANGE, None);
            runtime
                .store_mut()
                .set_animation_playing(state.ids[4], false);
            runtime.store_mut().set_animation_frame(state.ids[4], 0);
            runtime
                .store_mut()
                .set_object_font_size(state.ids[5], state.original_font + 14);
            state.sample = Some(index);
            state.stage = 1;
            state.since = frames;
        }
        1 if frames - state.since >= 12 => {
            commands
                .entity(runtime.entity_of(root(state, runtime)).unwrap())
                .insert(GlobalZIndex(500));
            state.views = state
                .ids
                .iter()
                .map(|&id| runtime.entity_of(id).unwrap())
                .collect();
            state.stage = 2;
            state.since = frames;
        }
        2 if frames - state.since >= 8 => {
            assert_eq!(
                fonts.get(state.views[5]).unwrap().font_size,
                bevy::text::FontSize::Px((state.original_font + 14) as f32)
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-gear-edited.png"));
            state.stage = 3;
            state.since = frames;
        }
        3 if frames - state.since >= 8 => {
            set_page(state, runtime, 1);
            state.stage = 4;
            state.since = frames;
        }
        4 if ready(state, frames, runtime) => {
            set_page(state, runtime, 0);
            state.stage = 5;
            state.since = frames;
        }
        5 if ready(state, frames, runtime) => {
            check(state, runtime, false, nodes, fonts);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-gear-restored.png"));
            state.stage = 6;
            state.since = frames;
        }
        6 if frames - state.since >= 8 => {
            set_page(state, runtime, -1);
            state.stage = 7;
            state.since = frames;
        }
        7 if ready(state, frames, runtime) => {
            runtime.store_mut().set_xy(state.ids[0], 232.0, 310.0);
            runtime
                .store_mut()
                .set_control_color(state.ids[2], GREEN, None);
            runtime
                .store_mut()
                .set_animation_playing(state.ids[4], false);
            runtime.store_mut().set_animation_frame(state.ids[4], 0);
            set_page(state, runtime, 0);
            state.stage = 8;
            state.since = frames;
        }
        8 if ready(state, frames, runtime) => {
            set_page(state, runtime, -1);
            state.stage = 9;
            state.since = frames;
        }
        9 if ready(state, frames, runtime) => {
            check(state, runtime, true, nodes, fonts);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-gear-null-page.png"));
            state.stage = 10;
            state.since = frames;
        }
        10 if frames - state.since >= 8 => {
            runtime.remove_root(commands, state.sample.unwrap());
            runtime.store_mut().set_visible(original, true);
            state.stage = 11;
            state.since = frames;
        }
        11 if frames - state.since >= 8 => {
            assert!(
                state
                    .ids
                    .iter()
                    .zip(&state.views)
                    .all(|(&id, &view)| runtime.entity_of(id).is_none()
                        && computed.get(view).is_err())
            );
            state.stage = 12;
            println!(
                "demo smoke: runtime XY/size/scale/alpha/color/animation edits survived native page roundtrips; font setter retained native no-capture behavior; null-page storage, entity reuse and disposal verified"
            );
        }
        _ => {}
    }
}
