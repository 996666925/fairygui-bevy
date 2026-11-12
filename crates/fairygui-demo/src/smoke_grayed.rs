use super::*;
use fairygui_core::widget::ControlData;

#[derive(Default)]
pub(super) struct State {
    stage: u8,
    since: u32,
    components: bool,
    buttons: bool,
    sample: Option<usize>,
    owners: Vec<GObjectId>,
    views: Vec<(GObjectId, Entity)>,
}
impl State {
    pub fn complete_for(&self, page: &str) -> bool {
        if page == "Component" {
            self.components
        } else {
            self.buttons
        }
    }
}

fn page_index(runtime: &GuiRuntime, id: GObjectId, name: &str) -> i32 {
    runtime
        .store()
        .object(id)
        .unwrap()
        .control()
        .component()
        .unwrap()
        .controller(name)
        .unwrap()
        .selected_index
}
fn check(state: &State, runtime: &GuiRuntime, page: &str, gray: bool) {
    for &owner in &state.owners {
        assert_eq!(runtime.store().object(owner).unwrap().grayed(), gray);
    }
    let child = |id, name| runtime.store().child_by_name(id, name).unwrap();
    if page == "Component" {
        for &owner in &state.owners {
            assert_eq!(
                runtime.store().object(child(owner, "n1")).unwrap().grayed(),
                gray
            );
            assert_eq!(
                runtime.store().object(child(owner, "n2")).unwrap().grayed(),
                gray
            );
        }
    } else {
        assert_eq!(
            runtime
                .store()
                .object(child(state.owners[0], "title"))
                .unwrap()
                .grayed(),
            gray
        );
        assert_eq!(
            page_index(runtime, state.owners[1], "grayed"),
            i32::from(gray)
        );
        assert!(
            !runtime
                .store()
                .object(child(state.owners[1], "title"))
                .unwrap()
                .grayed()
        );
        let ControlData::Button(button) =
            runtime.store().object(state.owners[2]).unwrap().control()
        else {
            panic!("button")
        };
        assert!(button.selected);
        assert_eq!(
            button
                .component
                .controller("button")
                .unwrap()
                .selected_page_name(),
            Some(if gray { "selectedDisabled" } else { "down" })
        );
        assert!(
            !runtime
                .store()
                .object(child(state.owners[2], "n4"))
                .unwrap()
                .grayed()
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn drive(
    state: &mut State,
    frames: u32,
    runtime: &mut GuiRuntime,
    assets: &mut Assets<FuiAsset>,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    commands: &mut Commands,
    original: GObjectId,
    page: &str,
) {
    match state.stage {
        0 => {
            let package = fairygui_core::FuiPackage::parse(
                "Basics",
                include_bytes!("../../fairygui-core/tests/fixtures/Grayed.fui"),
            )
            .unwrap();
            let handle = assets.add(FuiAsset {
                package: package.clone(),
                res_key: "ui/Basics".into(),
            });
            let object = runtime.store().object(original).unwrap();
            let position = Vec2::new(object.x(), object.y());
            let index = runtime
                .create_root(handle, &package, &format!("Demo_{page}"))
                .unwrap();
            let root = runtime.root_object(index).unwrap();
            runtime.store_mut().set_xy(root, position.x, position.y);
            runtime.store_mut().set_visible(original, false);
            for name in if page == "Component" {
                vec!["n17", "n20"]
            } else {
                vec!["n3", "n35", "n38"]
            } {
                state
                    .owners
                    .push(runtime.store().child_by_name(root, name).unwrap());
            }
            check(state, runtime, page, true);
            state.sample = Some(index);
            state.stage = 1;
            state.since = frames;
        }
        1 if frames - state.since >= 12 => {
            let root = runtime.root_object(state.sample.unwrap()).unwrap();
            commands
                .entity(runtime.entity_of(root).unwrap())
                .insert(GlobalZIndex(500));
            for &owner in &state.owners {
                for &child in runtime.store().object(owner).unwrap().children() {
                    state.views.push((child, runtime.entity_of(child).unwrap()));
                }
            }
            state.stage = 2;
            state.since = frames;
        }
        2 if frames - state.since >= 8 => {
            check(state, runtime, page, true);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(format!(
                    "target/fairygui-demo-grayed-{page}.png"
                )));
            state.stage = 3;
            state.since = frames;
        }
        3 if frames - state.since >= 8 => {
            // The source root still reads false. Native's first false assignment
            // nevertheless clears the independently serialized gray children.
            let root = runtime.root_object(state.sample.unwrap()).unwrap();
            assert!(!runtime.store().object(root).unwrap().grayed());
            runtime.store_mut().set_grayed(root, false);
            check(state, runtime, page, false);
            state.stage = 4;
            state.since = frames;
        }
        4 if frames - state.since >= 12 => {
            assert!(
                state
                    .views
                    .iter()
                    .all(|&(id, view)| runtime.entity_of(id) == Some(view))
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(format!(
                    "target/fairygui-demo-grayed-{page}-restored.png"
                )));
            state.stage = 5;
            state.since = frames;
        }
        5 if frames - state.since >= 8 => {
            runtime.remove_root(commands, state.sample.unwrap());
            runtime.store_mut().set_visible(original, true);
            state.stage = 6;
            state.since = frames;
        }
        6 if frames - state.since >= 8 => {
            assert!(
                state
                    .views
                    .iter()
                    .all(|&(id, view)| runtime.entity_of(id).is_none() && nodes.get(view).is_err())
            );
            if page == "Component" {
                state.components = true;
            } else {
                state.buttons = true;
            }
            state.owners.clear();
            state.views.clear();
            state.sample = None;
            state.stage = 0;
            println!(
                "demo smoke: {page} serialized gray, recursive first-false restoration, controller/disabled skin precedence, retained views and disposal verified"
            );
        }
        _ => {}
    }
}
