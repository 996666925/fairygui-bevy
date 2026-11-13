use super::*;
use fairygui_core::{geom::Size, package::ObjectType, widget::ControlData};

#[derive(Default)]
pub(super) struct State {
    stage: u8,
    since: u32,
    progress: Option<[GObjectId; 2]>,
    bars: Option<[(GObjectId, Size); 2]>,
    grid: Option<GObjectId>,
    loaders: Vec<GObjectId>,
    captured: bool,
}
impl State {
    pub fn complete(&self) -> bool {
        self.stage == 7
    }
}

pub(super) fn hold_progress(smoke: Res<Smoke>, mut runtime: NonSendMut<GuiRuntime>) {
    let state = &smoke.fills;
    let Some(progress) = state.progress.filter(|_| !state.complete()) else {
        return;
    };
    let values = if state.stage <= 1 {
        [0.25, 0.75]
    } else {
        [0.75, 0.25]
    };
    for (id, ratio) in progress.into_iter().zip(values) {
        let Some(ControlData::ProgressBar(data)) = runtime.store().object(id).map(|o| o.control())
        else {
            continue;
        };
        let value = data.min + (data.max - data.min) * ratio;
        runtime.store_mut().set_progress(id, value);
    }
}

pub(super) fn drive(
    state: &mut State,
    frames: u32,
    runtime: &mut GuiRuntime,
    commands: &mut Commands,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    root: GObjectId,
) {
    match state.stage {
        0 => {
            let progress = [
                runtime.store().child_by_name(root, "b0").unwrap(),
                runtime.store().child_by_name(root, "b1").unwrap(),
            ];
            state.bars = Some(progress.map(|id| {
                let bar = runtime.store().child_by_name(id, "bar").unwrap();
                (bar, runtime.store().object(bar).unwrap().size())
            }));
            state.progress = Some(progress);
            state.stage = 1;
            state.since = frames;
        }
        1 if frames - state.since >= 8 => {
            for (bar, size) in state.bars.unwrap() {
                assert_eq!(runtime.store().object(bar).unwrap().size(), size);
                assert!(
                    nodes
                        .get(runtime.entity_of(bar).unwrap())
                        .unwrap()
                        .0
                        .size()
                        .min_element()
                        > 0.0
                );
            }
            if !state.captured {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk("target/fairygui-demo-radial-cooldown.png"));
                state.captured = true;
                state.since = frames;
                return;
            }
            state.stage = 2;
            state.since = frames;
        }
        2 if frames - state.since >= 8 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-radial-cooldown-updated.png",
                ));
            state.stage = 3;
            state.since = frames;
        }
        3 if frames - state.since >= 8 => {
            let grid = runtime.store_mut().create(ObjectType::Component);
            runtime.store_mut().set_parent(grid, Some(root));
            runtime.store_mut().set_size(grid, 1000.0, 320.0);
            runtime.store_mut().set_xy(grid, 50.0, 290.0);
            for row in 0..3 {
                for column in 0..6 {
                    let loader = runtime.store_mut().create(ObjectType::Loader);
                    runtime.store_mut().set_parent(loader, Some(grid));
                    runtime
                        .store_mut()
                        .set_xy(loader, column as f32 * 165.0, row as f32 * 100.0);
                    runtime.store_mut().set_size(loader, 120.0, 80.0);
                    runtime.store_mut().set_loader_layout(loader, 4, 0, 0);
                    runtime
                        .store_mut()
                        .apply_icon_to(loader, "ui://Cooldown/11");
                    let method = [1, 2, 3, 4, 5, 5][column];
                    let origin = [0, 1, 2, 3, 0, 1][column];
                    runtime.store_mut().set_image_fill(
                        loader,
                        method,
                        origin,
                        column % 2 == 0,
                        [0.25, 0.5, 0.75][row],
                    );
                    state.loaders.push(loader);
                }
            }
            runtime.queue_subtree(grid);
            state.grid = Some(grid);
            state.stage = 4;
            state.since = frames;
        }
        4 if frames - state.since >= 12 => {
            for &loader in &state.loaders {
                assert_eq!(
                    runtime.loader_load_state(loader),
                    Some(GuiLoaderState::Ready)
                );
            }
            if let Some(entity) = runtime.entity_of(state.grid.unwrap()) {
                commands.entity(entity).insert(GlobalZIndex(500));
            }
            state.stage = 5;
            state.since = frames;
        }
        5 if frames - state.since >= 8 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-fill-masks.png"));
            state.stage = 6;
            state.since = frames;
        }
        6 if frames - state.since >= 8 => {
            runtime.store_mut().remove(state.grid.unwrap());
            state.stage = 7;
            println!(
                "demo smoke: native Cooldown progress updates retain bar geometry; horizontal/vertical/radial90/180/360 loader masks render at quarter/half/three-quarter fill"
            );
        }
        _ => {}
    }
}
