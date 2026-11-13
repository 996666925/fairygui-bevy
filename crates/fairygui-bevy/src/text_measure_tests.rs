use super::*;
use crate::prelude::*;
use bevy::ecs::system::SystemState;

fn fixture() -> (App, Entity, Entity, Handle<Font>) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<Assets<Font>>()
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let font = app.world().resource::<Assets<Font>>().reserve_handle();
    app.insert_resource(GuiFont(Some(font.clone())));
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: FuiPackage::parse(
                "Chat",
                include_bytes!("../../fairygui-demo/assets/ui/Chat.fui"),
            )
            .unwrap(),
            res_key: "ui/Chat".into(),
        });
    let owner = app.world_mut().spawn(FairyGuiRoot::new(asset, "Main")).id();
    app.update();
    let mut state = SystemState::<GuiQuery>::new(app.world_mut());
    let list = state
        .get(app.world())
        .unwrap()
        .child_by_name(owner, "list")
        .unwrap();
    (app, owner, list, font)
}
fn load_font(app: &mut App, handle: &Handle<Font>) {
    let font = Font::from_bytes(
        include_bytes!("../../fairygui-demo/assets/fonts/NotoSansSC-VF.ttf").to_vec(),
    );
    app.world_mut()
        .resource_mut::<Assets<Font>>()
        .insert(handle.id(), font)
        .unwrap();
}
fn config() -> GuiVirtualList {
    GuiVirtualList::new(60, |index, row| {
        row.set_icon("ui://Chat/r1");
        if let Some(mut msg) = row.child_by_path("msg") {
            let text = if index % 2 == 0 {
                "Hello".into()
            } else {
                "Long message with CJK 中文 and wrapping. ".repeat(9)
            };
            msg.set_text(&text);
        }
    })
    .with_item_provider(|index| {
        Some(
            if index % 2 == 0 {
                "chatLeft"
            } else {
                "chatRight"
            }
            .into(),
        )
    })
}

#[test]
fn synchronous_row_measurement_updates_bubbles_and_virtual_geometry_before_post_layout() {
    let (mut app, owner, list, font) = fixture();
    load_font(&mut app, &font);
    app.world_mut().entity_mut(list).insert(config());
    app.update();
    let mut queries = SystemState::<GuiQuery>::new(app.world_mut());
    let gui = queries.get(app.world()).unwrap();
    let rows = gui.list(list).unwrap().items().to_vec();
    assert!(!rows.is_empty());
    let mut ids = Vec::new();
    for &row in &rows {
        let index = app.world().get::<GuiListRow>(row).unwrap().index();
        let msg = gui.child_by_name(row, "msg").unwrap();
        let size = gui.transform(msg).unwrap().size();
        let bubble = gui
            .child_by_name(row, if index % 2 == 0 { "n0" } else { "n9" })
            .unwrap();
        let bubble_size = gui.transform(bubble).unwrap().size();
        assert!(
            size.x > 10. && size.x <= 663.,
            "measured text size {size:?}"
        );
        assert!((bubble_size.x - size.x - 37.).abs() < 0.1);
        assert!((bubble_size.y - size.y - 27.).abs() < 0.1);
        if index % 2 == 1 {
            assert!(
                size.y > 80.,
                "long messages must wrap before native row layout"
            );
            assert!(gui.transform(row).unwrap().size().y > 100.);
            assert!((gui.transform(msg).unwrap().position().x + size.x - 785.).abs() < 0.1);
            assert!(
                (gui.transform(bubble).unwrap().position().x + bubble_size.x - 811.).abs() < 0.1
            );
        }
        ids.push(app.world().get::<GObjectRef>(msg).unwrap().0);
    }
    let sizes: Vec<_> = rows
        .iter()
        .map(|&row| gui.transform(row).unwrap().size())
        .collect();
    app.update();
    let mut queries = SystemState::<GuiQuery>::new(app.world_mut());
    let gui = queries.get(app.world()).unwrap();
    assert_eq!(gui.list(list).unwrap().items(), rows);
    assert_eq!(
        rows.iter()
            .map(|&row| gui.transform(row).unwrap().size())
            .collect::<Vec<_>>(),
        sizes
    );
    app.world_mut().entity_mut(owner).despawn();
    app.update();
    app.update();
    let service = app.world().non_send::<GuiRuntime>().text_measurer.borrow();
    assert!(
        ids.iter()
            .all(|id| !service.requested.contains(id) && !service.cache.contains_key(id))
    );
}

#[test]
fn pending_font_retries_and_ecs_text_edits_do_not_use_stale_physical_node_sizes() {
    let (mut app, owner, list, font) = fixture();
    app.world_mut().entity_mut(list).insert(config());
    app.update();
    let row = app.world().get::<GuiListState>(list).unwrap().items()[0];
    let mut state = SystemState::<GuiQuery>::new(app.world_mut());
    let msg = state
        .get(app.world())
        .unwrap()
        .child_by_name(row, "msg")
        .unwrap();
    load_font(&mut app, &font);
    app.update();
    let mut state = SystemState::<GuiQuery>::new(app.world_mut());
    let before = state
        .get(app.world())
        .unwrap()
        .transform(msg)
        .unwrap()
        .size();
    assert!(before.x > 10.);
    let asset = app
        .world()
        .get::<FairyGuiRoot>(owner)
        .unwrap()
        .asset
        .clone();
    let standalone = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "chatLeft"))
        .id();
    app.update();
    let mut state = SystemState::<GuiQuery>::new(app.world_mut());
    let msg = state
        .get(app.world())
        .unwrap()
        .child_by_name(standalone, "msg")
        .unwrap();
    app.world_mut().write_message(GuiObjectCommand::new(
        msg,
        GuiObjectAction::SetText("Hello".into()),
    ));
    app.update();
    app.world_mut().write_message(GuiObjectCommand::new(
        msg,
        GuiObjectAction::SetText("A much longer replacement".into()),
    ));
    // A stale/HiDPI node must never overwrite the synchronous logical size.
    app.world_mut().entity_mut(msg).insert(ComputedNode {
        size: Vec2::splat(4000.),
        inverse_scale_factor: 0.5,
        ..default()
    });
    app.update();
    let mut state = SystemState::<GuiQuery>::new(app.world_mut());
    let after = state
        .get(app.world())
        .unwrap()
        .transform(msg)
        .unwrap()
        .size();
    assert!(after.x > before.x && after.x < 663.);
    assert_eq!(after.y, before.y);
    app.world_mut().write_message(GuiObjectCommand::new(
        msg,
        GuiObjectAction::SetText(String::new()),
    ));
    app.update();
    let mut state = SystemState::<GuiQuery>::new(app.world_mut());
    assert_eq!(
        state
            .get(app.world())
            .unwrap()
            .transform(msg)
            .unwrap()
            .size(),
        Vec2::ZERO
    );
}

#[test]
fn automatic_text_size_and_relations_are_current_before_the_first_initialize() {
    #[derive(Resource, Default)]
    struct Initial(usize);
    fn inspect(
        roots: Query<(Entity, &FairyGuiRoot), Added<GuiRootView>>,
        gui: GuiQuery,
        nodes: Query<&Node>,
        mut initial: ResMut<Initial>,
    ) {
        for (owner, source) in &roots {
            if source.item_name != "chatLeft" {
                continue;
            }
            let text = gui.child_by_name(owner, "msg").unwrap();
            let bubble = gui.child_by_name(owner, "n0").unwrap();
            assert_eq!(gui.transform(text).unwrap().size(), Vec2::ZERO);
            assert_eq!(gui.transform(bubble).unwrap().size(), Vec2::new(37., 27.));
            let node = nodes.get(text).unwrap();
            assert_eq!((node.width, node.height), (Val::Px(0.), Val::Px(0.)));
            initial.0 += 1;
        }
    }
    let (mut app, owner, _, font) = fixture();
    load_font(&mut app, &font);
    app.init_resource::<Initial>()
        .add_systems(Update, inspect.in_set(GuiSystems::Initialize));
    let asset = app
        .world()
        .get::<FairyGuiRoot>(owner)
        .unwrap()
        .asset
        .clone();
    app.world_mut().spawn(FairyGuiRoot::new(asset, "chatLeft"));
    app.update();
    assert_eq!(app.world().resource::<Initial>().0, 1);
}

#[test]
fn initialize_and_post_layout_text_edits_measure_in_the_same_frame_without_an_ensure_command() {
    #[derive(Resource)]
    struct Target(Entity);
    #[derive(Resource, Default)]
    struct Observed(bool);
    fn initialize(target: Res<Target>, gui: GuiQuery, mut commands: Commands) {
        if let Some(text) = gui.child_by_name(target.0, "msg") {
            commands.write_message(GuiObjectCommand::new(
                text,
                GuiObjectAction::SetText("Long message 中文 ".repeat(40)),
            ));
        }
    }
    fn post_layout(
        target: Res<Target>,
        gui: GuiQuery,
        mut observed: ResMut<Observed>,
        mut commands: Commands,
    ) {
        if let Some(text) = gui.child_by_name(target.0, "msg") {
            assert!(gui.transform(text).unwrap().size().y > 100.);
            observed.0 = true;
            commands.write_message(GuiObjectCommand::new(
                text,
                GuiObjectAction::SetText("Short".into()),
            ));
        }
    }
    let (mut app, owner, _, font) = fixture();
    load_font(&mut app, &font);
    let asset = app
        .world()
        .get::<FairyGuiRoot>(owner)
        .unwrap()
        .asset
        .clone();
    let root = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "chatLeft"))
        .id();
    app.insert_resource(Target(root))
        .init_resource::<Observed>()
        .add_systems(Update, initialize.in_set(GuiSystems::Initialize))
        .add_systems(Update, post_layout.in_set(GuiSystems::PostLayout));
    app.update();
    assert!(app.world().resource::<Observed>().0);
    let mut queries = SystemState::<GuiQuery>::new(app.world_mut());
    let gui = queries.get(app.world()).unwrap();
    let text = gui.child_by_name(root, "msg").unwrap();
    let bubble = gui.child_by_name(root, "n0").unwrap();
    let size = gui.transform(text).unwrap().size();
    assert!(size.x > 10. && size.x < 150. && size.y < 50.);
    assert_eq!(
        gui.transform(bubble).unwrap().size(),
        size + Vec2::new(37., 27.)
    );
    let node = app.world().get::<Node>(text).unwrap();
    assert_eq!(
        (node.width, node.height),
        (Val::Px(size.x), Val::Px(size.y))
    );
}

#[test]
fn automatic_height_keeps_width_and_fixed_text_keeps_exported_bounds() {
    let (mut app, _, _, font) = fixture();
    load_font(&mut app, &font);
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: FuiPackage::parse(
                "Basics",
                include_bytes!("../../fairygui-demo/assets/ui/Basics.fui"),
            )
            .unwrap(),
            res_key: "ui/Basics".into(),
        });
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "Demo_Text"))
        .id();
    app.update();
    let (height, fixed) = {
        let mut queries = SystemState::<GuiQuery>::new(app.world_mut());
        let gui = queries.get(app.world()).unwrap();
        let height = gui.child_by_name(owner, "n24").unwrap();
        assert_eq!(gui.transform(height).unwrap().size(), Vec2::new(299., 0.));
        (height, gui.child_by_name(owner, "n2").unwrap())
    };
    for text in ["Repeated text 中文 ".repeat(30), String::new()] {
        app.world_mut().write_message(GuiObjectCommand::new(
            height,
            GuiObjectAction::SetText(text.clone()),
        ));
        app.world_mut().write_message(GuiObjectCommand::new(
            fixed,
            GuiObjectAction::SetText(text.clone()),
        ));
        app.update();
        let size = app.world().get::<GuiTransform>(height).unwrap().size();
        assert_eq!(size.x, 299.);
        if text.is_empty() {
            assert_eq!(size.y, 0.);
        } else {
            assert!(size.y > 100.);
        }
        assert_eq!(
            app.world().get::<GuiTransform>(fixed).unwrap().size(),
            Vec2::new(337., 50.)
        );
    }
}
