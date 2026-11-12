use super::*;

#[derive(Default)]
pub(super) struct State {
    stage: u8,
    since: u32,
    root: Option<usize>,
    combo: Option<GObjectId>,
    dropdown: Option<GObjectId>,
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
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    window: &mut Window,
    commands: &mut Commands,
    mouse: &mut ButtonInput<MouseButton>,
    original_root: GObjectId,
) {
    use fairygui_core::package::FuiPackage;
    match state.stage {
        0 => {
            let host = FuiPackage::parse(
                "Basics",
                include_bytes!("../../fairygui-core/tests/fixtures/PopupHost.fui"),
            )
            .unwrap();
            let theme = FuiPackage::parse(
                "Basics",
                include_bytes!("../../fairygui-core/tests/fixtures/PopupTheme.fui"),
            )
            .unwrap();
            let host_handle = assets.add(FuiAsset {
                package: host.clone(),
                res_key: "ui/Basics".into(),
            });
            let theme_handle = assets.add(FuiAsset {
                package: theme.clone(),
                res_key: "ui/Basics".into(),
            });
            let index = runtime
                .create_root(host_handle, &host, "Demo_ComboBox")
                .unwrap();
            let root = runtime.root_object(index).unwrap();
            let combo = runtime.store().child_by_name(root, "n1").unwrap();
            let count = runtime.store().len();
            assert!(
                runtime
                    .show_combo_dropdown(commands, combo, &host)
                    .is_none()
            );
            assert_eq!(
                runtime.store().len(),
                count,
                "unregistered foreign template cannot leak objects"
            );
            runtime.register_package(theme_handle, &theme);
            let original = runtime.store().object(original_root).unwrap();
            let at = Vec2::new(original.x(), original.y());
            runtime.store_mut().set_xy(root, at.x, at.y);
            runtime.store_mut().set_visible(original_root, false);
            state.root = Some(index);
            state.combo = Some(combo);
            state.stage = 1;
            state.since = frames;
        }
        1 if frames - state.since >= 8 => {
            let (_, transform) = nodes
                .get(runtime.entity_of(state.combo.unwrap()).unwrap())
                .unwrap();
            window.set_cursor_position(Some(
                transform.transform_point2(Vec2::ZERO) / window.scale_factor(),
            ));
            state.stage = 2;
            state.since = frames;
        }
        2 | 5 if frames - state.since >= 4 => {
            mouse.press(MouseButton::Left);
            state.stage += 1;
            state.since = frames;
        }
        3 if frames - state.since >= 8 => {
            let combo = state.combo.unwrap();
            let popup = runtime
                .store()
                .combo_popup(combo)
                .expect("foreign dropdown opened through actual pointer routing");
            let row = runtime.store().child_at(popup.list, 0).unwrap();
            let dropdown_source = runtime.asset_handle_for(popup.dropdown).unwrap();
            let row_source = runtime.asset_handle_for(row).unwrap();
            assert_eq!(
                assets.get(&dropdown_source).unwrap().package.id(),
                "theme001"
            );
            assert_eq!(assets.get(&row_source).unwrap().package.id(), "host0001");
            assert_eq!(runtime.store().list_item_count(popup.list), 8);
            state.dropdown = Some(popup.dropdown);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-cross-package-popup.png"));
            mouse.release(MouseButton::Left);
            state.stage = 4;
            state.since = frames;
        }
        4 if frames - state.since >= 4 => {
            let popup = runtime.store().combo_popup(state.combo.unwrap()).unwrap();
            let row = runtime.store().child_at(popup.list, 2).unwrap();
            let (_, transform) = nodes.get(runtime.entity_of(row).unwrap()).unwrap();
            window.set_cursor_position(Some(
                transform.transform_point2(Vec2::ZERO) / window.scale_factor(),
            ));
            state.stage = 5;
            state.since = frames;
        }
        6 if frames - state.since >= 4 => {
            mouse.release(MouseButton::Left);
            state.stage = 7;
            state.since = frames;
        }
        7 if frames - state.since >= 8 => {
            assert!(runtime.popup_stack().is_empty());
            assert!(
                matches!(runtime.store().object(state.combo.unwrap()).unwrap().control(),
                fairygui_core::widget::ControlData::ComboBox(data) if data.selected_index == 2 && data.title == "Item 3")
            );
            let title = runtime
                .store()
                .child_by_name(state.combo.unwrap(), "title")
                .unwrap();
            let rendered = nodes.get(runtime.entity_of(title).unwrap()).unwrap().0;
            assert!(rendered.size().x > 0.0 && rendered.size().y > 0.0);
            assert_eq!(
                runtime
                    .store()
                    .object(title)
                    .unwrap()
                    .control()
                    .as_text_field()
                    .unwrap()
                    .text,
                "Item 3"
            );
            runtime.remove_root(commands, state.root.take().unwrap());
            runtime.store_mut().set_visible(original_root, true);
            state.stage = 8;
            state.since = frames;
        }
        8 if frames - state.since >= 4 => {
            assert!(
                !runtime.store().contains(state.dropdown.unwrap()),
                "disposing the owner reclaims a foreign cached dropdown"
            );
            state.stage = 9;
            println!(
                "demo smoke: foreign dropdown and returning host rows rendered, selected through actual input, and reclaimed after owner disposal; registration retry verified"
            );
        }
        _ => {}
    }
}
