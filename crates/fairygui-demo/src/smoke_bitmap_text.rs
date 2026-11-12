use super::*;
use fairygui_bevy::runtime::{BitmapDecorations, BitmapGlyphs, BitmapTextMetrics};

#[derive(Default)]
pub(super) struct State {
    stage: u8,
    since: u32,
    sample: Option<usize>,
    owners: Vec<GObjectId>,
    views: Vec<Entity>,
    old_glyphs: Vec<Entity>,
    old_decorations: Vec<Entity>,
    final_decorations: Vec<Entity>,
    sizes: Vec<fairygui_core::Size>,
}
impl State {
    pub fn complete(&self) -> bool {
        self.stage == 9
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn drive(
    state: &mut State,
    frames: u32,
    runtime: &mut GuiRuntime,
    assets: &mut Assets<FuiAsset>,
    glyphs: &Query<&BitmapGlyphs>,
    metrics: &Query<&BitmapTextMetrics>,
    decorations: &Query<&BitmapDecorations>,
    nodes: &Query<&Node>,
    commands: &mut Commands,
    original: GObjectId,
) {
    match state.stage {
        0 => {
            let package = fairygui_core::FuiPackage::parse(
                "Basics",
                include_bytes!("../../fairygui-core/tests/fixtures/BitmapText.fui"),
            )
            .unwrap();
            let handle = assets.add(FuiAsset {
                package: package.clone(),
                res_key: "ui/Basics".into(),
            });
            let object = runtime.store().object(original).unwrap();
            let position = Vec2::new(object.x(), object.y());
            let index = runtime.create_root(handle, &package, "Demo_Text").unwrap();
            let root = runtime.root_object(index).unwrap();
            runtime.store_mut().set_xy(root, position.x, position.y);
            runtime.store_mut().set_visible(original, false);
            state.owners = ["n0", "n2", "n3", "n4", "n7", "n8", "n10", "n11", "n17"]
                .map(|name| runtime.store().child_by_name(root, name).unwrap())
                .to_vec();
            state.sample = Some(index);
            for &owner in &state.owners {
                runtime.store_mut().set_object_ubb_enabled(owner, false);
                runtime.store_mut().set_object_html_enabled(owner, false);
                runtime.store_mut().set_object_underline(owner, true);
            }
            state.stage = 1;
            state.since = frames;
        }
        1 if frames - state.since >= 16 => {
            let root = runtime.root_object(state.sample.unwrap()).unwrap();
            commands
                .entity(runtime.entity_of(root).unwrap())
                .insert(GlobalZIndex(500));
            for &owner in &state.owners {
                let view = runtime.entity_of(owner).unwrap();
                let glyphs = glyphs.get(view).unwrap();
                let lines = &decorations.get(view).unwrap().0;
                assert!(!lines.is_empty());
                assert!(lines.iter().all(|&e| {
                    let node = nodes.get(e).unwrap();
                    matches!(node.width, Val::Px(w) if w>0.0)
                        && matches!(node.height, Val::Px(h) if h>=1.0)
                }));
                state.old_decorations.extend(lines);
                assert!(!glyphs.0.is_empty());
                let measured = metrics.get(view).unwrap();
                let object = runtime.store().object(owner).unwrap();
                let data = object.control().as_text_field().unwrap();
                if matches!(data.auto_size, 1 | 2) {
                    assert_eq!(object.size(), measured.object_size);
                }
                assert!(glyphs.0.iter().all(|&view| nodes.get(view).is_ok()));
                state.views.push(view);
                state.old_glyphs.extend(&glyphs.0);
                state.sizes.push(object.size());
            }
            state.stage = 2;
            state.since = frames;
        }
        2 if frames - state.since >= 12 => {
            for (&owner, &size) in state.owners.iter().zip(&state.sizes) {
                assert_eq!(runtime.store().object(owner).unwrap().size(), size);
            }
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-bitmap-text.png"));
            state.stage = 3;
            state.since = frames;
        }
        3 if frames - state.since >= 10 => {
            for (i, &owner) in state.owners.iter().enumerate() {
                runtime.store_mut().set_object_underline(owner, i % 2 != 0);
                runtime
                    .store_mut()
                    .set_object_font_size(owner, if i % 2 == 0 { 36 } else { 24 });
                if i % 3 == 0 {
                    runtime.store_mut().set_size(owner, 160.0, 150.0);
                }
            }
            state.stage = 4;
            state.since = frames;
        }
        4 if frames - state.since >= 16 => {
            assert!(state.old_decorations.iter().all(|&e| nodes.get(e).is_err()));
            for (i, &view) in state.views.iter().enumerate() {
                assert_eq!(decorations.get(view).unwrap().0.is_empty(), i % 2 == 0);
            }
            assert!(
                state
                    .old_glyphs
                    .iter()
                    .all(|&view| nodes.get(view).is_err())
            );
            assert!(
                state
                    .owners
                    .iter()
                    .zip(&state.views)
                    .all(|(&id, &view)| runtime.entity_of(id) == Some(view))
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-bitmap-text-updated.png"));
            state.stage = 5;
            state.since = frames;
        }
        5 if frames - state.since >= 10 => {
            runtime.store_mut().set_object_text(state.owners[0], "");
            runtime.store_mut().set_object_text(state.owners[1], "?😀?");
            state.stage = 6;
            state.since = frames;
        }
        6 if frames - state.since >= 10 => {
            assert!(glyphs.get(state.views[0]).unwrap().0.is_empty());
            assert!(glyphs.get(state.views[1]).unwrap().0.is_empty());
            assert!(decorations.get(state.views[1]).unwrap().0.is_empty());
            runtime
                .store_mut()
                .set_object_text(state.owners[0], "12345");
            runtime.store_mut().set_object_text(state.owners[1], "UI#");
            state.stage = 7;
            state.since = frames;
        }
        7 if frames - state.since >= 10 => {
            assert_eq!(glyphs.get(state.views[0]).unwrap().0.len(), 5);
            assert_eq!(glyphs.get(state.views[1]).unwrap().0.len(), 3);
            assert!(!decorations.get(state.views[1]).unwrap().0.is_empty());
            for &view in &state.views {
                state
                    .final_decorations
                    .extend(&decorations.get(view).unwrap().0);
            }
            runtime.remove_root(commands, state.sample.unwrap());
            runtime.store_mut().set_visible(original, true);
            state.stage = 8;
            state.since = frames;
        }
        8 if frames - state.since >= 10 => {
            assert!(state.views.iter().all(|&view| nodes.get(view).is_err()));
            assert!(
                state
                    .final_decorations
                    .iter()
                    .all(|&e| nodes.get(e).is_err())
            );
            state.stage = 9;
            println!(
                "demo smoke: bitmap image/atlas fonts scale, round advances, wrap/align/shrink/ellipsis and auto-size in logical pixels; native plain underlines, runtime toggles, resize/font updates, empty/missing recovery, retained owners and decoration disposal verified"
            );
        }
        _ => {}
    }
}
