use crate::prelude::*;
use bevy::ecs::system::SystemState;
use bevy::prelude::*;

#[derive(Component, Clone, Copy)]
enum Fixture {
    Content,
    Virtual,
    Tree,
}
#[derive(Component)]
struct Configured {
    view: Entity,
    list: Entity,
}
#[derive(Resource, Default)]
struct Visits {
    initialize: usize,
    post_layout: usize,
}

fn initialize(
    mut commands: Commands,
    roots: Query<(Entity, &GuiRootView, &Fixture), Changed<GuiRootView>>,
    gui: GuiQuery,
    mut visits: ResMut<Visits>,
) {
    for (owner, view, fixture) in &roots {
        // Fresh native hierarchy/geometry/text are available before content
        // preparation; attach configs without borrowing GuiRuntime.
        assert_eq!(gui.view(owner), Some(view.0));
        let name = match fixture {
            Fixture::Content => "mailList",
            Fixture::Virtual => "list",
            Fixture::Tree => "tree2",
        };
        let list = gui.child_by_name(owner, name).unwrap();
        assert!(gui.transform(list).is_some());
        match fixture {
            Fixture::Content => {
                commands
                    .entity(list)
                    .insert(GuiListContent::new(10, |index, row| {
                        row.set_text(&format!("Initial {index}"))
                    }));
            }
            Fixture::Virtual => {
                commands.entity(list).insert(
                    GuiVirtualList::new(5, |index, row| row.set_text(&format!("Initial {index}")))
                        .looping(),
                );
            }
            Fixture::Tree => {
                let mut folder = GuiTreeItem::folder(
                    "folder",
                    "Folder",
                    vec![GuiTreeItem::leaf("file", "Initial 1")],
                );
                folder.expanded = Some(true);
                commands.entity(list).insert(GuiTreeContent::new(vec![
                    folder,
                    GuiTreeItem::leaf("last", "Initial 2"),
                ]));
            }
        }
        commands
            .entity(owner)
            .insert(Configured { view: view.0, list });
        commands.write_message(GuiObjectCommand::new(
            owner,
            GuiObjectAction::SetPosition(Vec2::new(33., 47.)),
        ));
        visits.initialize += 1;
    }
}

fn post_layout(
    roots: Query<(&Configured, &Fixture), Changed<Configured>>,
    gui: GuiQuery,
    mut visits: ResMut<Visits>,
    mut commands: MessageWriter<GuiObjectCommand>,
) {
    for (configured, fixture) in &roots {
        let list = gui.list(configured.list).unwrap();
        let count = match fixture {
            Fixture::Content => 10,
            Fixture::Virtual => 5,
            Fixture::Tree => 3,
        };
        assert_eq!(list.item_count(), count);
        assert!(
            !list.items().is_empty(),
            "Initialize data must have rows before the first PostLayout"
        );
        for &row in list.items() {
            assert!(gui.transform(row).is_some());
            assert!(!gui.text(row).unwrap().is_empty());
            commands.write(GuiObjectCommand::new(row, GuiObjectAction::SetAlpha(0.5)));
        }
        assert_eq!(
            gui.transform(configured.view).unwrap().position(),
            Vec2::new(33., 47.)
        );
        visits.post_layout += 1;
    }
}

#[test]
fn initialize_configures_normal_virtual_and_tree_rows_on_the_first_ready_frame_and_on_replacement()
{
    for (name, bytes, fixture) in [
        (
            "ListEffect",
            &include_bytes!("../../fairygui-demo/assets/ui/ListEffect.fui")[..],
            Fixture::Content,
        ),
        (
            "LoopList",
            &include_bytes!("../../fairygui-demo/assets/ui/LoopList.fui")[..],
            Fixture::Virtual,
        ),
        (
            "TreeView",
            &include_bytes!("../../fairygui-demo/assets/ui/TreeView.fui")[..],
            Fixture::Tree,
        ),
    ] {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            FairyGuiPlugin,
        ))
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<Visits>()
        .add_message::<bevy::input::mouse::MouseWheel>()
        .add_systems(Update, initialize.in_set(GuiSystems::Initialize))
        .add_systems(Update, post_layout.in_set(GuiSystems::PostLayout));
        let handle = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
        let owner = app
            .world_mut()
            .spawn((FairyGuiRoot::new(handle.clone(), "Main"), fixture))
            .id();
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(app.world().resource::<Visits>().initialize, 0);
        let package = crate::core::FuiPackage::parse(name, bytes).unwrap();
        app.world_mut()
            .resource_mut::<Assets<FuiAsset>>()
            .insert(
                handle.id(),
                FuiAsset {
                    package,
                    res_key: format!("ui/{name}"),
                },
            )
            .unwrap();
        app.update();
        let old_view = app.world().get::<GuiRootView>(owner).unwrap().0;
        assert_eq!(app.world().resource::<Visits>().initialize, 1);
        assert_eq!(app.world().resource::<Visits>().post_layout, 1);
        let list = app.world().get::<Configured>(owner).unwrap().list;
        let rows = app
            .world()
            .get::<GuiListState>(list)
            .unwrap()
            .items()
            .to_vec();
        let mut queries = SystemState::<GuiQuery>::new(app.world_mut());
        let gui = queries.get(app.world()).unwrap();
        for &row in &rows {
            assert_eq!(gui.display(row).unwrap().alpha(), 0.5);
        }
        app.update();
        assert_eq!(app.world().resource::<Visits>().initialize, 1);
        assert_eq!(app.world().resource::<Visits>().post_layout, 1);
        assert_eq!(app.world().get::<GuiListState>(list).unwrap().items(), rows);

        // Reinsert the root source on the same owner: the new view must receive
        // configuration immediately and old row entities must be reclaimed.
        app.world_mut().entity_mut(owner).remove::<FairyGuiRoot>();
        app.update();
        app.world_mut()
            .entity_mut(owner)
            .insert(FairyGuiRoot::new(handle, "Main"));
        app.update();
        assert_ne!(app.world().get::<GuiRootView>(owner).unwrap().0, old_view);
        assert_eq!(app.world().resource::<Visits>().initialize, 2);
        assert_eq!(app.world().resource::<Visits>().post_layout, 2);
        assert!(rows.iter().all(|&row| app.world().get_entity(row).is_err()));
    }
}
