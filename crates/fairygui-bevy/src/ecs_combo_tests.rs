use super::*;
use crate::prelude::*;
use bevy::ecs::system::{RunSystemOnce, SystemState};
use fairygui_core::{object::EventKind, package::FuiPackage};

fn load(app: &mut App, handle: &Handle<FuiAsset>) {
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(
            handle.id(),
            FuiAsset {
                package: FuiPackage::parse(
                    "Basics",
                    include_bytes!("../../fairygui-demo/assets/ui/Basics.fui"),
                )
                .unwrap(),
                res_key: "ui/Basics".into(),
            },
        )
        .unwrap();
}
fn fixture(loaded: bool) -> (App, Entity, Handle<FuiAsset>) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let handle = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    if loaded {
        load(&mut app, &handle);
    }
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(handle.clone(), "ComboBox"))
        .id();
    (app, owner, handle)
}
fn send(app: &mut App, target: Entity, action: GuiObjectAction) {
    app.world_mut()
        .write_message(GuiObjectCommand::new(target, action));
}
fn snapshot(app: &mut App, owner: Entity) -> GuiComboBoxState {
    SystemState::<GuiQuery>::new(app.world_mut())
        .get(app.world())
        .unwrap()
        .combo_box(owner)
        .unwrap()
        .clone()
}
fn view(app: &App, owner: Entity) -> Entity {
    app.world().get::<GuiRootView>(owner).unwrap().0
}
fn open(app: &mut App, owner: Entity) {
    let object = app.world().get::<GuiRootInstance>(owner).unwrap().object;
    app.world_mut()
        .run_system_once(
            move |mut commands: Commands,
                  assets: Res<Assets<FuiAsset>>,
                  mut gui: NonSendMut<GuiRuntime>| {
                let handle = gui.asset_handle_for(object).unwrap();
                let package = &assets.get(&handle).unwrap().package;
                gui.ensure_root(fairygui_core::Size::new(800., 600.));
                assert!(
                    gui.show_combo_dropdown(&mut commands, object, package)
                        .is_some()
                );
            },
        )
        .unwrap();
    app.update();
}

#[test]
fn queued_combo_setters_apply_before_initialize_without_rebuilding_and_keep_snapshot_ticks() {
    #[derive(Resource, Default)]
    struct Initial(usize);
    fn inspect(
        roots: Query<Entity, Added<GuiRootView>>,
        gui: GuiQuery,
        mut initial: ResMut<Initial>,
    ) {
        for root in &roots {
            let combo = gui.combo_box(root).unwrap();
            assert_eq!(combo.selected_index(), 1);
            assert_eq!(combo.value(), Some("two"));
            assert_eq!(gui.text(root), Some("Two"));
            assert_eq!(combo.visible_item_count(), 3);
            assert_eq!(combo.popup_direction(), GuiPopupDirection::Up);
            initial.0 += 1;
        }
    }
    let (mut app, owner, handle) = fixture(false);
    app.init_resource::<Initial>()
        .add_systems(Update, inspect.in_set(GuiSystems::Initialize));
    send(
        &mut app,
        owner,
        GuiObjectAction::SetComboItems(vec!["One".into(), "Two".into()]),
    );
    send(
        &mut app,
        owner,
        GuiObjectAction::SetComboValues(vec!["one".into(), "two".into()]),
    );
    send(&mut app, owner, GuiObjectAction::SetComboSelection(1));
    send(
        &mut app,
        owner,
        GuiObjectAction::SetComboVisibleItemCount(3),
    );
    send(
        &mut app,
        owner,
        GuiObjectAction::SetComboPopupDirection(GuiPopupDirection::Up),
    );
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(app.world().resource::<Initial>().0, 0);
    load(&mut app, &handle);
    app.update();
    let entity = view(&app, owner);
    assert_eq!(app.world().resource::<Initial>().0, 1);
    let tick = app
        .world()
        .entity(entity)
        .get_ref::<GuiComboBoxState>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .entity(entity)
            .get_ref::<GuiComboBoxState>()
            .unwrap()
            .last_changed(),
        tick
    );
    send(
        &mut app,
        owner,
        GuiObjectAction::SetComboValue(Some("one".into())),
    );
    app.update();
    assert_eq!(snapshot(&mut app, owner).selected_index(), 0);
    assert_eq!(view(&app, owner), entity);
}

#[test]
fn combo_value_icon_arrays_and_items_setters_preserve_native_independent_arrays() {
    let (mut app, owner, _) = fixture(true);
    send(
        &mut app,
        owner,
        GuiObjectAction::SetComboItems(vec!["A".into(), "B".into()]),
    );
    send(
        &mut app,
        owner,
        GuiObjectAction::SetComboValues(vec!["".into(), "b".into()]),
    );
    send(&mut app, owner, GuiObjectAction::SetIcon("custom".into()));
    send(
        &mut app,
        owner,
        GuiObjectAction::SetComboValue(Some("b".into())),
    );
    app.update();
    assert_eq!(
        app.world()
            .get::<GuiIcon>(view(&app, owner))
            .unwrap()
            .value(),
        "custom"
    );
    send(
        &mut app,
        owner,
        GuiObjectAction::SetComboIcons(Some(vec![Some("first".into())])),
    );
    app.update();
    assert_eq!(snapshot(&mut app, owner).icons().unwrap().len(), 1);
    send(
        &mut app,
        owner,
        GuiObjectAction::SetComboValue(Some("absent".into())),
    );
    app.update();
    let state = snapshot(&mut app, owner);
    assert_eq!(state.selected_index(), -1);
    assert!(state.value().is_none());
    send(&mut app, owner, GuiObjectAction::SetComboValue(None));
    app.update();
    assert_eq!(snapshot(&mut app, owner).value(), Some(""));
    send(&mut app, owner, GuiObjectAction::SetComboSelection(10));
    app.update();
    assert_eq!(snapshot(&mut app, owner).selected_index(), 10);
    send(
        &mut app,
        owner,
        GuiObjectAction::SetComboItems(vec!["Only".into()]),
    );
    app.update();
    assert_eq!(snapshot(&mut app, owner).selected_index(), 0);
    send(&mut app, owner, GuiObjectAction::SetComboItems(Vec::new()));
    app.update();
    let state = snapshot(&mut app, owner);
    assert!(state.items().is_empty());
    assert_eq!(state.values(), ["", "b"]);
    assert_eq!(state.selected_index(), -1);
}

#[test]
fn combo_observers_read_current_selection_and_programmatic_edits_do_not_emit_change() {
    #[derive(Resource, Default)]
    struct Seen(Vec<(i32, String)>);
    fn observe(event: On<GuiEvent>, gui: GuiQuery, mut seen: ResMut<Seen>) {
        if event.event.kind == EventKind::StateChanged && !event.event.bubbling {
            let state = gui.combo_box(event.entity).unwrap();
            seen.0
                .push((state.selected_index(), state.value().unwrap().to_owned()));
        }
    }
    let (mut app, owner, _) = fixture(true);
    app.init_resource::<Seen>();
    send(
        &mut app,
        owner,
        GuiObjectAction::SetComboItems(vec!["A".into(), "B".into()]),
    );
    send(
        &mut app,
        owner,
        GuiObjectAction::SetComboValues(vec!["a".into(), "b".into()]),
    );
    app.update();
    let entity = view(&app, owner);
    app.world_mut().entity_mut(entity).observe(observe);
    send(&mut app, owner, GuiObjectAction::SetComboSelection(1));
    app.update();
    assert!(app.world().resource::<Seen>().0.is_empty());
    let object = app.world().get::<GObjectRef>(entity).unwrap().0;
    {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        runtime.store_mut().select_combo_index(object, 0);
        runtime.dispatch(EventKind::StateChanged, object);
    }
    app.update();
    assert_eq!(app.world().resource::<Seen>().0, [(0, "a".into())]);
}

#[test]
fn combo_popup_direction_uses_native_up_one_down_two_codes() {
    let (mut app, owner, _) = fixture(true);
    send(
        &mut app,
        owner,
        GuiObjectAction::SetPosition(Vec2::new(100., 200.)),
    );
    send(
        &mut app,
        owner,
        GuiObjectAction::SetComboItems(vec!["A".into(), "B".into()]),
    );
    send(
        &mut app,
        owner,
        GuiObjectAction::SetComboPopupDirection(GuiPopupDirection::Up),
    );
    app.update();
    open(&mut app, owner);
    let pane = snapshot(&mut app, owner).dropdown().unwrap();
    assert!(app.world().get::<GuiTransform>(pane).unwrap().position().y < 200.);
    app.world_mut().write_message(GuiPopupCommand::HideAll);
    app.update();
    send(
        &mut app,
        owner,
        GuiObjectAction::SetComboPopupDirection(GuiPopupDirection::Down),
    );
    app.update();
    open(&mut app, owner);
    assert_eq!(snapshot(&mut app, owner).dropdown(), Some(pane));
    assert!(app.world().get::<GuiTransform>(pane).unwrap().position().y > 200.);
}

#[test]
fn combo_popup_reuses_views_refreshes_options_on_reopen_and_is_disposed_with_owner() {
    let (mut app, owner, _) = fixture(true);
    send(
        &mut app,
        owner,
        GuiObjectAction::SetComboItems(vec!["A".into(), "B".into()]),
    );
    send(
        &mut app,
        owner,
        GuiObjectAction::SetComboValues(vec!["a".into(), "b".into()]),
    );
    app.update();
    open(&mut app, owner);
    let state = snapshot(&mut app, owner);
    assert!(state.is_open());
    let (dropdown, list) = (state.dropdown().unwrap(), state.list().unwrap());
    app.world_mut().write_message(GuiPopupCommand::HideAll);
    app.update();
    assert!(!snapshot(&mut app, owner).is_open());
    send(
        &mut app,
        owner,
        GuiObjectAction::SetComboItems(vec!["New".into()]),
    );
    app.update();
    open(&mut app, owner);
    let state = snapshot(&mut app, owner);
    assert_eq!(
        (state.dropdown(), state.list()),
        (Some(dropdown), Some(list))
    );
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    let gui = query.get(app.world()).unwrap();
    let rows = gui.get(list).unwrap().children();
    assert_eq!(rows.len(), 1);
    assert_eq!(gui.text(rows[0]), Some("New"));
    assert_eq!(gui.get(rows[0]).unwrap().name(), "a");
    app.world_mut().despawn(owner);
    app.update();
    assert!(app.world().get_entity(dropdown).is_err());
    assert!(app.world().get_entity(list).is_err());
}
