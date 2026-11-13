use super::*;
use bevy::ecs::system::SystemState;

#[test]
fn visible_mail_rows_start_once_with_native_stagger_and_offscreen_rows_remain_unanimated() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .register_gui_extension_at::<MailItem>("ui://ListEffect/mailItem")
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>()
    .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::from_millis(100),
    ))
    .add_systems(Update, initialize.in_set(GuiSystems::Initialize))
    .add_systems(Update, play.in_set(GuiSystems::PostLayout));
    fn initialize(mut commands: Commands, roots: Query<Entity, Added<GuiRootView>>, gui: GuiQuery) {
        for owner in &roots {
            if let Some(list) = gui.child_by_name(owner, "mailList") {
                commands.entity(list).insert((Pending, content()));
            }
        }
    }
    let package = fairygui_bevy::core::FuiPackage::parse(
        "ListEffect",
        include_bytes!("../assets/ui/ListEffect.fui"),
    )
    .unwrap();
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package,
            res_key: "ui/ListEffect".into(),
        });
    let owner = app.world_mut().spawn(FairyGuiRoot::new(asset, "Main")).id();
    app.update();
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    let list = query
        .get(app.world())
        .unwrap()
        .child_by_name(owner, "mailList")
        .unwrap();
    // The very first app.update must already expose the configured rows with
    // their initial transition state; no unconfigured frame may be rendered.
    let rows = app
        .world()
        .get::<GuiListState>(list)
        .unwrap()
        .items()
        .to_vec();
    assert_eq!(rows.len(), 10);
    assert!(app.world().get::<Pending>(list).is_none());
    let gui = query.get(app.world()).unwrap();
    for (index, &row) in rows.iter().enumerate() {
        assert_eq!(gui.text(row), Some("Mail title here"));
        assert_eq!(
            gui.controller(row, "IsRead").unwrap().selected_index(),
            i32::from(index % 2 == 0)
        );
        assert_eq!(
            gui.controller(row, "c1").unwrap().selected_index(),
            i32::from(index % 3 == 0)
        );
        assert_eq!(
            gui.text(gui.child_by_name(row, "timeText").unwrap()),
            Some("5 Nov 2015 16:24:33")
        );
        assert_eq!(
            gui.transition(row, "t0").unwrap().play_generation(),
            u64::from(index < 4)
        );
        assert_eq!(
            gui.display(row).unwrap().visible(),
            index == 0 || index >= 4
        );
    }
    assert!((gui.transform(rows[0]).unwrap().position().x - 391.).abs() < 0.01);
    app.update();
    let gui = query.get(app.world()).unwrap();
    let x = gui.transform(rows[0]).unwrap().position().x;
    assert!(x > 0. && x < 391.);
    assert!(
        !gui.display(rows[1]).unwrap().visible(),
        "second row still in delay"
    );
    app.update();
    let gui = query.get(app.world()).unwrap();
    assert!(gui.display(rows[1]).unwrap().visible());
    assert!(!gui.display(rows[2]).unwrap().visible());
    let mut completions = Vec::new();
    for _ in 0..20 {
        app.update();
        completions.extend(
            app.world_mut()
                .resource_mut::<Messages<GuiTransitionComplete>>()
                .drain(),
        );
    }
    assert_eq!(completions.len(), 4);
    let gui = query.get(app.world()).unwrap();
    for (index, &row) in rows.iter().enumerate() {
        assert_eq!(
            gui.transform(row).unwrap().position(),
            Vec2::new(0., index as f32 * 125.)
        );
        assert!(gui.display(row).unwrap().visible());
        assert!(!gui.transition(row, "t0").unwrap().is_playing());
    }
    app.world_mut().write_message(GuiObjectCommand::new(
        list,
        GuiObjectAction::ScrollTo {
            position: Vec2::new(0., 500.),
            animated: false,
        },
    ));
    app.update();
    assert_eq!(
        query
            .get(app.world())
            .unwrap()
            .transition(rows[4], "t0")
            .unwrap()
            .play_generation(),
        0
    );
    app.world_mut().despawn(owner);
    app.update();
    assert!(rows.iter().all(|&row| app.world().get_entity(row).is_err()));
}
