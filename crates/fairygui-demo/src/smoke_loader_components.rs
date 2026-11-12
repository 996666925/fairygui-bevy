use super::*;

#[derive(Default)]
pub(super) struct State {
    stage: u8,
    since: u32,
    loader: Option<GObjectId>,
    content: Option<GObjectId>,
    content_view: Option<Entity>,
    original: Option<(String, fairygui_core::geom::Size)>,
    clicks: usize,
    captured_scale: bool,
    captured_resize: bool,
    error_sign: Option<GObjectId>,
    error_view: Option<Entity>,
    captured_error: bool,
}

impl State {
    pub fn complete(&self) -> bool {
        self.stage == 19
    }
}

pub(super) fn observe_loaded_button(event: On<GuiEvent>, mut smoke: ResMut<Smoke>) {
    if event.event().event.kind == fairygui_core::object::EventKind::Click {
        smoke.loader_components.clicks += 1;
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn drive(
    state: &mut State,
    frames: u32,
    runtime: &mut GuiRuntime,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    images: &Query<&ImageNode>,
    commands: &mut Commands,
    mouse: &mut ButtonInput<MouseButton>,
    window: &mut Window,
    root: GObjectId,
) {
    use fairygui_core::{geom::Size, widget::ControlData};
    match state.stage {
        0 => {
            let loader = runtime.store().child_by_name(root, "n2").unwrap();
            let object = runtime.store().object(loader).unwrap();
            let ControlData::Loader(data) = object.control() else {
                unreachable!()
            };
            state.original = Some((data.url.clone().unwrap(), object.size()));
            runtime
                .store_mut()
                .apply_icon_to(loader, "ui://Basics/Component12");
            runtime.store_mut().set_size(loader, 260.0, 170.0);
            runtime.store_mut().set_loader_layout(loader, 1, 1, 1);
            state.loader = Some(loader);
            state.stage = 1;
            state.since = frames;
        }
        1 if frames - state.since >= 8 => {
            let loader = state.loader.unwrap();
            let component = runtime.loader_component(loader).unwrap();
            let object = runtime.store().object(component).unwrap();
            assert_eq!(object.size(), Size::new(400.0, 297.0));
            assert!((object.scale_x() - 170.0 / 297.0).abs() < 0.0001);
            let view = runtime.entity_of(component).unwrap();
            assert!(nodes.get(view).unwrap().0.size().x > 0.0);
            state.content = Some(component);
            state.content_view = Some(view);
            if !state.captured_scale {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(
                        "target/fairygui-demo-loader-component-scale.png",
                    ));
                state.captured_scale = true;
                state.since = frames;
                return;
            }
            runtime.store_mut().set_loader_use_resize(loader, true);
            runtime.store_mut().set_loader_layout(loader, 4, 0, 0);
            state.stage = 2;
            state.since = frames;
        }
        2 if frames - state.since >= 8 => {
            let component = runtime.loader_component(state.loader.unwrap()).unwrap();
            assert_eq!(Some(component), state.content);
            assert_eq!(runtime.entity_of(component), state.content_view);
            assert_eq!(
                runtime.store().object(component).unwrap().size(),
                Size::new(260.0, 170.0)
            );
            let background = runtime.store().child_by_name(component, "n9").unwrap();
            assert_eq!(
                runtime.store().object(background).unwrap().size(),
                Size::new(260.0, 170.0)
            );
            assert!(images.get(runtime.entity_of(background).unwrap()).is_ok());
            if !state.captured_resize {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(
                        "target/fairygui-demo-loader-component-resize.png",
                    ));
                state.captured_resize = true;
                state.since = frames;
                return;
            }
            runtime
                .store_mut()
                .set_loader_use_resize(state.loader.unwrap(), false);
            runtime
                .store_mut()
                .apply_icon_to(state.loader.unwrap(), "ui://Basics/PopupMenuItem");
            state.stage = 3;
            state.since = frames;
        }
        3 if frames - state.since >= 8 => {
            assert!(!runtime.store().contains(state.content.unwrap()));
            let button = runtime.loader_component(state.loader.unwrap()).unwrap();
            runtime.store_mut().set_object_text(button, "Loaded button");
            if let Some(entity) = runtime.entity_of(button) {
                commands.entity(entity).observe(observe_loaded_button);
            }
            state.content = Some(button);
            state.content_view = runtime.entity_of(button);
            let (_, transform) = nodes.get(state.content_view.unwrap()).unwrap();
            window.set_cursor_position(Some(
                transform.transform_point2(Vec2::ZERO) / window.scale_factor(),
            ));
            state.stage = 4;
            state.since = frames;
        }
        4 if frames - state.since >= 4 => {
            mouse.press(MouseButton::Left);
            state.stage = 5;
            state.since = frames;
        }
        5 if frames - state.since >= 4 => {
            mouse.release(MouseButton::Left);
            state.stage = 6;
            state.since = frames;
        }
        6 if frames - state.since >= 8 => {
            let button = state.content.unwrap();
            assert_eq!(
                state.clicks, 1,
                "loaded button triggers one application GuiEvent observer"
            );
            assert_eq!(runtime.store().button_selected(button), Some(true));
            runtime
                .store_mut()
                .apply_icon_to(state.loader.unwrap(), "ui://Basics/VtSeperator");
            state.stage = 7;
            state.since = frames;
        }
        7 if frames - state.since >= 8 => {
            assert!(runtime.loader_component(state.loader.unwrap()).is_none());
            assert!(!runtime.store().contains(state.content.unwrap()));
            runtime
                .store_mut()
                .apply_icon_to(state.loader.unwrap(), "ui://Missing/Component12");
            state.stage = 8;
            state.since = frames;
        }
        8 if frames - state.since >= 4 => {
            assert!(runtime.loader_component(state.loader.unwrap()).is_none());
            runtime.store_mut().apply_icon_to(state.loader.unwrap(), "");
            state.stage = 9;
            state.since = frames;
        }
        9 if frames - state.since >= 4 => {
            let loader = state.loader.unwrap();
            let (url, size) = state.original.take().unwrap();
            runtime.store_mut().apply_icon_to(loader, &url);
            runtime
                .store_mut()
                .set_size(loader, size.width, size.height);
            runtime.store_mut().set_loader_layout(loader, 1, 0, 0);
            state.stage = 10;
            state.since = frames;
        }
        10 if frames - state.since >= 8 => {
            assert!(runtime.loader_component(state.loader.unwrap()).is_none());
            let loader = state.loader.unwrap();
            let object = runtime.store().object(loader).unwrap();
            let ControlData::Loader(data) = object.control() else {
                unreachable!()
            };
            state.original = Some((data.url.clone().unwrap(), object.size()));
            runtime.set_loader_error_sign(Some("ui://Basics/PopupMenuItem"));
            runtime.store_mut().set_loader_show_error_sign(loader, true);
            runtime.store_mut().set_size(loader, 260.0, 100.0);
            runtime
                .store_mut()
                .apply_icon_to(loader, "ui://Missing/SmokeError");
            state.stage = 11;
            state.since = frames;
        }
        11 if frames - state.since >= 8 => {
            let loader = state.loader.unwrap();
            assert_eq!(
                runtime.loader_load_state(loader),
                Some(GuiLoaderState::Failed(GuiLoaderFailure::MissingResource))
            );
            let sign = runtime.loader_error_sign(loader).unwrap();
            runtime
                .store_mut()
                .set_object_text(sign, "Resource unavailable");
            assert_eq!(
                runtime.store().object(sign).unwrap().size(),
                Size::new(260.0, 100.0)
            );
            state.error_sign = Some(sign);
            state.error_view = runtime.entity_of(sign);
            assert!(nodes.get(state.error_view.unwrap()).unwrap().0.size().x > 0.0);
            state.stage = 12;
            state.since = frames;
        }
        12 if frames - state.since >= 8 => {
            if !state.captured_error {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk("target/fairygui-demo-loader-error.png"));
                state.captured_error = true;
                state.since = frames;
                return;
            }
            let loader = state.loader.unwrap();
            runtime.store_mut().set_size(loader, 300.0, 120.0);
            runtime
                .store_mut()
                .set_loader_show_error_sign(loader, false);
            state.stage = 13;
            state.since = frames;
        }
        13 if frames - state.since >= 8 => {
            let loader = state.loader.unwrap();
            assert_eq!(runtime.loader_error_sign(loader), state.error_sign);
            assert_eq!(
                runtime
                    .store()
                    .object(state.error_sign.unwrap())
                    .unwrap()
                    .size(),
                Size::new(260.0, 100.0)
            );
            runtime.retry_loader(loader);
            state.stage = 14;
            state.since = frames;
        }
        14 if frames - state.since >= 8 => {
            let loader = state.loader.unwrap();
            assert!(runtime.loader_error_sign(loader).is_none());
            assert!(runtime.store().contains(state.error_sign.unwrap()));
            runtime.store_mut().set_loader_show_error_sign(loader, true);
            runtime.retry_loader(loader);
            state.stage = 15;
            state.since = frames;
        }
        15 if frames - state.since >= 8 => {
            let loader = state.loader.unwrap();
            assert_eq!(runtime.loader_error_sign(loader), state.error_sign);
            assert_eq!(
                runtime.entity_of(state.error_sign.unwrap()),
                state.error_view
            );
            assert_eq!(
                runtime
                    .store()
                    .object(state.error_sign.unwrap())
                    .unwrap()
                    .size(),
                Size::new(300.0, 120.0)
            );
            runtime
                .store_mut()
                .apply_icon_to(loader, "ui://Basics/Component12");
            state.stage = 16;
            state.since = frames;
        }
        16 if frames - state.since >= 8 => {
            let loader = state.loader.unwrap();
            assert_eq!(
                runtime.loader_load_state(loader),
                Some(GuiLoaderState::Ready)
            );
            assert!(runtime.loader_error_sign(loader).is_none());
            assert!(runtime.loader_component(loader).is_some());
            runtime.store_mut().apply_icon_to(loader, "");
            runtime.set_loader_error_sign(None);
            state.stage = 17;
            state.since = frames;
        }
        17 if frames - state.since >= 8 => {
            assert_eq!(
                runtime.loader_load_state(state.loader.unwrap()),
                Some(GuiLoaderState::Empty)
            );
            let loader = state.loader.unwrap();
            let (url, size) = state.original.take().unwrap();
            runtime.store_mut().apply_icon_to(loader, &url);
            runtime
                .store_mut()
                .set_size(loader, size.width, size.height);
            state.stage = 18;
            state.since = frames;
        }
        18 if frames - state.since >= 4 => {
            state.stage = 19;
            println!(
                "demo smoke: loader component scale/resize/input/disposal and error-sign rendering, captured size, flag/retry semantics, retained model/view reuse and recovery passed"
            );
        }
        _ => {}
    }
}
