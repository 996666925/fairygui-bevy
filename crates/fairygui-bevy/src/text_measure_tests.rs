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
            msg.ensure_size_correct();
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
    app.world_mut()
        .write_message(GuiObjectCommand::new(msg, GuiObjectAction::EnsureTextSize));
    app.update();
    app.world_mut().write_message(GuiObjectCommand::new(
        msg,
        GuiObjectAction::SetText("A much longer replacement".into()),
    ));
    app.world_mut()
        .write_message(GuiObjectCommand::new(msg, GuiObjectAction::EnsureTextSize));
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
