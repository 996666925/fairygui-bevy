use crate::prelude::*;
use crate::properties::sync_property;
use bevy::prelude::*;
use fairygui_core::{object::GroupLayoutSettings, package::ObjectType};

#[test]
fn group_component_merges_direct_edits_and_restores_invalid_fields() {
    let mut world = World::new();
    world.init_non_send::<GuiRuntime>();
    let group = world
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .create(ObjectType::Group);
    let entity = world.spawn(GObjectRef(group)).id();
    sync_property::<GuiGroupLayout>(&mut world);
    assert_eq!(
        world.get::<GuiGroupLayout>(entity).unwrap().main_grid_index,
        None
    );
    world.get_mut::<GuiGroupLayout>(entity).unwrap().column_gap = 13;
    world
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_group_layout_settings(
            group,
            GroupLayoutSettings {
                main_grid_index: Some(2),
                main_grid_min_size: 80.,
                ..Default::default()
            },
        );
    sync_property::<GuiGroupLayout>(&mut world);
    let settings = *world.get::<GuiGroupLayout>(entity).unwrap();
    assert_eq!(settings.column_gap, 13);
    assert_eq!(settings.main_grid_index, Some(2));
    assert_eq!(settings.main_grid_min_size, 80.);
    {
        let mut settings = world.get_mut::<GuiGroupLayout>(entity).unwrap();
        settings.main_grid_min_size = f32::NAN;
        settings.line_gap = -5;
    }
    sync_property::<GuiGroupLayout>(&mut world);
    let settings = world.get::<GuiGroupLayout>(entity).unwrap();
    assert_eq!(settings.main_grid_min_size, 80.);
    assert_eq!(settings.line_gap, -5);
    // Component policy belongs only to groups.
    let image = world
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .create(ObjectType::Image);
    let entity = world
        .spawn((GObjectRef(image), GuiGroupLayout::default()))
        .id();
    sync_property::<GuiGroupLayout>(&mut world);
    assert!(world.get::<GuiGroupLayout>(entity).is_none());
}

#[test]
fn initialize_and_ecs_resizing_allocate_group_space_before_post_layout() {
    use bevy::ecs::system::SystemState;
    #[derive(Component)]
    struct Panel;
    #[derive(Resource, Default)]
    struct Visits(usize);
    fn initialize(
        roots: Query<Entity, (With<Panel>, Added<GuiRootView>)>,
        gui: GuiQuery,
        mut commands: Commands,
    ) {
        for owner in &roots {
            let group = gui.child_by_name(owner, "n4").unwrap();
            let mut transform = *gui.transform(group).unwrap();
            transform.size = Vec2::new(240., 80.);
            commands.entity(group).insert((
                GuiGroupLayout {
                    layout: GroupLayoutType::Horizontal,
                    auto_size_disabled: true,
                    main_grid_index: Some(0),
                    column_gap: 7,
                    ..default()
                },
                transform,
            ));
        }
    }
    fn post_layout(roots: Query<Entity, With<Panel>>, gui: GuiQuery, mut visits: ResMut<Visits>) {
        for owner in &roots {
            let Some(group) = gui.child_by_name(owner, "n4") else {
                continue;
            };
            let image = gui.child_by_name(owner, "n1").unwrap();
            let movie = gui.child_by_name(owner, "n3").unwrap();
            let group_size = gui.transform(group).unwrap().size();
            let movie_size = gui.transform(movie).unwrap().size();
            assert_eq!(
                gui.transform(image).unwrap().size().x,
                group_size.x - movie_size.x - 7.
            );
            assert_eq!(gui.group_layout(group).unwrap().main_grid_index, Some(0));
            visits.0 += 1;
        }
    }
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>()
    .init_resource::<Visits>()
    .add_systems(Update, initialize.in_set(GuiSystems::Initialize))
    .add_systems(Update, post_layout.in_set(GuiSystems::PostLayout));
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: fairygui_core::FuiPackage::parse(
                "ModalWaiting",
                include_bytes!("../../fairygui-demo/assets/ui/ModalWaiting.fui"),
            )
            .unwrap(),
            res_key: "ui/ModalWaiting".into(),
        });
    let owner = app
        .world_mut()
        .spawn((
            FairyGuiRoot::new(asset, "WindowModalWaiting").with_size(Vec2::new(300., 300.)),
            Panel,
        ))
        .id();
    app.update();
    assert_eq!(app.world().resource::<Visits>().0, 1);
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    let gui = query.get(app.world()).unwrap();
    let group = gui.child_by_name(owner, "n4").unwrap();
    let image = gui.child_by_name(owner, "n1").unwrap();
    let movie = gui.child_by_name(owner, "n3").unwrap();
    let initial_movie_width = gui.transform(movie).unwrap().size().x;
    assert_eq!(
        gui.transform(group).unwrap().position(),
        Vec2::new(30., 110.)
    );
    let mut transform = *gui.transform(group).unwrap();
    transform.size.x = 330.;
    app.world_mut().entity_mut(group).insert(transform);
    app.update();
    let gui = query.get(app.world()).unwrap();
    assert_eq!(
        gui.transform(group).unwrap().position(),
        Vec2::new(-15., 110.)
    );
    assert_eq!(
        gui.transform(image).unwrap().size().x,
        330. - initial_movie_width - 7.
    );
    assert_eq!(gui.transform(movie).unwrap().size().x, initial_movie_width);
    let stable: Vec<_> = [group, image, movie]
        .into_iter()
        .map(|id| *gui.transform(id).unwrap())
        .collect();
    app.update();
    let gui = query.get(app.world()).unwrap();
    assert_eq!(
        [group, image, movie]
            .into_iter()
            .map(|id| *gui.transform(id).unwrap())
            .collect::<Vec<_>>(),
        stable
    );
    assert_eq!(app.world().resource::<Visits>().0, 3);
}
