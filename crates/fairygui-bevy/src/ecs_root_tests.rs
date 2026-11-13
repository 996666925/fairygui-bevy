use super::*;
use crate::FairyGuiPlugin;
use crate::ecs::{GuiDisplay, GuiObjectInfo, GuiQuery, GuiText, GuiTransform};
use bevy::ecs::system::SystemState;

fn app() -> (App, Handle<FuiAsset>) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let asset = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    (app, asset)
}

fn load(app: &mut App, asset: &Handle<FuiAsset>) {
    let package = FuiPackage::parse(
        "MainMenu",
        include_bytes!("../../fairygui-demo/assets/ui/MainMenu.fui"),
    )
    .unwrap();
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(
            asset.id(),
            FuiAsset {
                package,
                res_key: "ui/MainMenu".into(),
            },
        )
        .unwrap();
}

fn send(app: &mut App, target: Entity, action: GuiObjectAction) {
    app.world_mut()
        .write_message(GuiObjectCommand::new(target, action));
}

fn assert_position(app: &App, owner: Entity, expected: Vec2) -> GObjectId {
    let root = app.world().get::<GuiRootInstance>(owner).unwrap().object;
    let runtime = app.world().non_send::<GuiRuntime>();
    let object = runtime.store().object(root).unwrap();
    assert_eq!((object.x(), object.y()), (expected.x, expected.y));
    let view = runtime.entity_of(root).unwrap();
    let node = app.world().get::<Node>(view).unwrap();
    assert_eq!(node.left, Val::Px(expected.x));
    assert_eq!(node.top, Val::Px(expected.y));
    root
}

#[test]
fn startup_root_commands_apply_before_the_first_view_is_spawned() {
    #[derive(Resource)]
    struct Source(Handle<FuiAsset>);
    #[derive(Component)]
    struct Menu;
    fn spawn(mut commands: Commands, source: Res<Source>) {
        let owner = commands
            .spawn((FairyGuiRoot::new(source.0.clone(), "Main"), Menu))
            .id();
        commands.write_message(GuiObjectCommand::new(
            owner,
            GuiObjectAction::SetPosition(Vec2::new(37., 42.)),
        ));
        commands.write_message(GuiObjectCommand::new(
            owner,
            GuiObjectAction::SetVisible(false),
        ));
    }

    let (mut app, asset) = app();
    load(&mut app, &asset);
    app.insert_resource(Source(asset))
        .add_systems(Startup, spawn);
    app.update();
    let owner = app
        .world_mut()
        .query_filtered::<Entity, With<Menu>>()
        .single(app.world())
        .unwrap();
    let root = assert_position(&app, owner, Vec2::new(37., 42.));
    let view = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(root)
        .unwrap();
    assert_eq!(
        app.world().get::<Visibility>(view),
        Some(&Visibility::Hidden)
    );
}

#[test]
fn delayed_root_commands_outlive_message_retention_preserve_order_and_apply_once() {
    let (mut app, asset) = app();
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), "Main"))
        .id();
    send(
        &mut app,
        owner,
        GuiObjectAction::SetPosition(Vec2::new(10., 20.)),
    );
    send(&mut app, owner, GuiObjectAction::SetVisible(false));
    app.update();
    send(
        &mut app,
        owner,
        GuiObjectAction::SetPosition(Vec2::new(40., 50.)),
    );
    for _ in 0..4 {
        app.update();
    }
    assert!(app.world().get::<GuiRootInstance>(owner).is_none());
    load(&mut app, &asset);
    send(
        &mut app,
        owner,
        GuiObjectAction::SetPosition(Vec2::new(60., 70.)),
    );
    app.update();
    let root = assert_position(&app, owner, Vec2::new(60., 70.));
    let view = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(root)
        .unwrap();
    assert_eq!(
        app.world().get::<Visibility>(view),
        Some(&Visibility::Hidden)
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_xy(root, 90., 100.);
    app.update();
    assert_position(&app, owner, Vec2::new(90., 100.));
}

#[test]
fn replacing_or_removing_a_pending_root_cancels_old_commands() {
    for replace in [0, 1, 2] {
        let (mut app, asset) = app();
        let owner = app
            .world_mut()
            .spawn(FairyGuiRoot::new(asset.clone(), "Main"))
            .id();
        send(
            &mut app,
            owner,
            GuiObjectAction::SetPosition(Vec2::new(37., 42.)),
        );
        app.update();
        let loaded_asset = if replace == 1 {
            app.world_mut()
                .get_mut::<FairyGuiRoot>(owner)
                .unwrap()
                .item_name = "CloseButton".into();
            asset
        } else if replace == 2 {
            let replacement = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
            app.world_mut()
                .get_mut::<FairyGuiRoot>(owner)
                .unwrap()
                .asset = replacement.clone();
            replacement
        } else {
            // Even removal and reinsertion between updates starts a new lifetime.
            app.world_mut().entity_mut(owner).remove::<FairyGuiRoot>();
            app.world_mut()
                .entity_mut(owner)
                .insert(FairyGuiRoot::new(asset.clone(), "Main"));
            asset
        };
        load(&mut app, &loaded_asset);
        app.update();
        assert_position(&app, owner, Vec2::ZERO);
    }
}

#[test]
fn ready_root_replacement_applies_new_commands_to_its_new_model() {
    let (mut app, asset) = app();
    load(&mut app, &asset);
    let owner = app.world_mut().spawn(FairyGuiRoot::new(asset, "Main")).id();
    app.update();
    let old = assert_position(&app, owner, Vec2::ZERO);
    app.world_mut()
        .get_mut::<FairyGuiRoot>(owner)
        .unwrap()
        .item_name = "CloseButton".into();
    send(
        &mut app,
        owner,
        GuiObjectAction::SetPosition(Vec2::new(37., 42.)),
    );
    app.update();
    let new = assert_position(&app, owner, Vec2::new(37., 42.));
    assert_ne!(new, old);
    assert!(!app.world().non_send::<GuiRuntime>().store().contains(old));
}

#[test]
fn pending_commands_are_discarded_when_the_owner_is_despawned() {
    let (mut app, asset) = app();
    let old = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), "Main"))
        .id();
    send(
        &mut app,
        old,
        GuiObjectAction::SetPosition(Vec2::new(37., 42.)),
    );
    app.update();
    app.world_mut().despawn(old);
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), "Main"))
        .id();
    load(&mut app, &asset);
    app.update();
    assert_ne!(owner, old);
    assert_position(&app, owner, Vec2::ZERO);
}

#[test]
fn ecs_queries_follow_root_loading_replacement_removal_and_retained_views() {
    let (mut app, asset) = app();
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), "Main"))
        .id();
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    app.update();
    assert_eq!(query.get(app.world()).unwrap().view(owner), None);
    load(&mut app, &asset);
    app.update();
    let view = {
        let gui = query.get(app.world()).unwrap();
        let view = gui.view(owner).unwrap();
        assert_eq!(gui.view(view), Some(view));
        let button = gui.child_by_name(owner, "n1").unwrap();
        assert_eq!(gui.get(button).unwrap().name(), "n1");
        assert_eq!(
            gui.get(button).unwrap().kind(),
            fairygui_core::package::ObjectType::Button
        );
        assert_eq!(gui.get(button).unwrap().parent(), Some(view));
        assert!(gui.child_by_path(owner, "n1/title").is_some());
        assert!(gui.child_by_path(owner, "n1/missing").is_none());
        assert_eq!(gui.child_by_path(owner, ""), Some(view));
        view
    };
    let unchanged = app
        .world()
        .entity(view)
        .get_ref::<GuiObjectInfo>()
        .unwrap()
        .last_changed();
    app.world_mut()
        .get_mut::<FairyGuiRoot>(owner)
        .unwrap()
        .active = false;
    app.update();
    assert_eq!(query.get(app.world()).unwrap().view(owner), Some(view));
    assert_eq!(
        app.world()
            .entity(view)
            .get_ref::<GuiObjectInfo>()
            .unwrap()
            .last_changed(),
        unchanged
    );

    app.world_mut()
        .get_mut::<FairyGuiRoot>(owner)
        .unwrap()
        .item_name = "CloseButton".into();
    app.update();
    let replacement = query.get(app.world()).unwrap().view(owner).unwrap();
    assert_ne!(replacement, view);
    assert!(query.get(app.world()).unwrap().get(view).is_none());
    app.world_mut().entity_mut(owner).remove::<FairyGuiRoot>();
    app.update();
    assert!(app.world().get::<GuiRootView>(owner).is_none());
    let gui = query.get(app.world()).unwrap();
    assert!(gui.view(owner).is_none());
    assert!(gui.get(replacement).is_none());
    assert!(gui.transform(replacement).is_none());
}

#[test]
fn ecs_queries_preserve_logical_order_and_track_dynamic_names_and_parenting() {
    use fairygui_core::package::ObjectType;
    let (mut app, asset) = app();
    load(&mut app, &asset);
    let owner = app.world_mut().spawn(FairyGuiRoot::new(asset, "Main")).id();
    app.update();
    let root = app.world().get::<GuiRootInstance>(owner).unwrap().object;
    let (branch, nested, direct) = {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        let branch = runtime.store_mut().create(ObjectType::Component);
        runtime.store_mut().set_name(branch, "branch");
        runtime.store_mut().set_parent(branch, Some(root));
        let nested = runtime.store_mut().create(ObjectType::Graph);
        runtime.store_mut().set_name(nested, "needle");
        runtime.store_mut().set_parent(nested, Some(branch));
        let direct = runtime.store_mut().create(ObjectType::Graph);
        runtime.store_mut().set_name(direct, "needle");
        runtime.store_mut().set_parent(direct, Some(root));
        runtime.queue_subtree(branch);
        runtime.queue_subtree(direct);
        (branch, nested, direct)
    };
    app.update();
    let (view, branch_view, nested_view, direct_view) = {
        let runtime = app.world().non_send::<GuiRuntime>();
        (
            runtime.entity_of(root).unwrap(),
            runtime.entity_of(branch).unwrap(),
            runtime.entity_of(nested).unwrap(),
            runtime.entity_of(direct).unwrap(),
        )
    };
    let decorative = app
        .world_mut()
        .spawn((Name::new("needle"), ChildOf(view)))
        .id();
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    {
        let gui = query.get(app.world()).unwrap();
        assert_eq!(gui.find_by_name(owner, "needle"), Some(direct_view));
        assert_eq!(gui.child_by_path(owner, "branch/needle"), Some(nested_view));
        assert!(!gui.get(view).unwrap().children().contains(&decorative));
        assert!(gui.get(decorative).is_none());
    }

    {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        runtime.store_mut().set_name(direct, "renamed");
        runtime.store_mut().add_child_at(root, nested, 0);
    }
    app.update();
    {
        let gui = query.get(app.world()).unwrap();
        assert_eq!(gui.find_by_name(owner, "needle"), Some(nested_view));
        assert_eq!(gui.child_by_name(owner, "renamed"), Some(direct_view));
        assert_eq!(gui.get(nested_view).unwrap().parent(), Some(view));
        assert_eq!(
            gui.get(view).unwrap().children().first(),
            Some(&nested_view)
        );
        assert!(gui.child_by_path(owner, "branch/needle").is_none());
    }
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .detach_from_parent(branch);
    app.update();
    let gui = query.get(app.world()).unwrap();
    assert!(gui.child_by_name(owner, "branch").is_none());
    assert_eq!(gui.get(branch_view).unwrap().parent(), None);
    assert_eq!(gui.view(branch_view), Some(branch_view));
}

#[test]
fn ecs_geometry_preserves_logical_coordinates_and_updates_views_in_the_same_sync() {
    let (mut app, asset) = app();
    load(&mut app, &asset);
    let owner = app.world_mut().spawn(FairyGuiRoot::new(asset, "Main")).id();
    app.update();
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    let (view, child, original_child) = {
        let gui = query.get(app.world()).unwrap();
        let child = gui.child_by_name(owner, "n1").unwrap();
        (
            gui.view(owner).unwrap(),
            child,
            *gui.transform(child).unwrap(),
        )
    };
    for action in [
        GuiObjectAction::SetPosition(Vec2::new(-10., 20.)),
        GuiObjectAction::SetSize(Vec2::new(200., 80.)),
        GuiObjectAction::SetScale(Vec2::new(-2., 0.5)),
        GuiObjectAction::SetPivot {
            pivot: Vec2::new(0.25, 0.75),
            as_anchor: true,
        },
        GuiObjectAction::SetRotation(90.),
    ] {
        send(&mut app, owner, action);
    }
    app.update();
    let geometry = {
        let gui = query.get(app.world()).unwrap();
        let geometry = *gui.transform(owner).unwrap();
        assert_eq!(gui.transform(view), Some(&geometry));
        assert_eq!(geometry.position(), Vec2::new(-10., 20.));
        assert_eq!(geometry.size(), Vec2::new(200., 80.));
        assert_eq!(geometry.scale(), Vec2::new(-2., 0.5));
        assert_eq!(geometry.skew(), Vec2::ZERO);
        assert_eq!(geometry.pivot(), Vec2::new(0.25, 0.75));
        assert!(geometry.pivot_as_anchor());
        assert_eq!(geometry.rotation(), 90.);
        assert_eq!(gui.transform(child), Some(&original_child));
        geometry
    };
    let node = app.world().get::<Node>(view).unwrap();
    assert_eq!(node.left, Val::Px(-10.));
    assert_eq!(node.top, Val::Px(20.));
    assert_eq!(node.width, Val::Px(200.));
    assert_eq!(node.height, Val::Px(80.));
    let rendered = app.world().get::<UiTransform>(view).unwrap();
    let (Val::Px(x), Val::Px(y)) = (rendered.translation.x, rendered.translation.y) else {
        panic!("pivot compensation must use logical pixels");
    };
    assert!((x + 90.).abs() < 0.001 && (y + 140.).abs() < 0.001);
    assert_eq!(rendered.scale, geometry.scale());
    assert_eq!(rendered.rotation, Rot2::degrees(90.));

    let unchanged = app
        .world()
        .entity(view)
        .get_ref::<GuiTransform>()
        .unwrap()
        .last_changed();
    send(
        &mut app,
        owner,
        GuiObjectAction::SetPivot {
            pivot: Vec2::new(f32::NAN, 0.),
            as_anchor: false,
        },
    );
    app.world_mut()
        .get_mut::<FairyGuiRoot>(owner)
        .unwrap()
        .active = false;
    app.update();
    assert_eq!(
        query.get(app.world()).unwrap().transform(owner),
        Some(&geometry)
    );
    assert_eq!(
        app.world()
            .entity(view)
            .get_ref::<GuiTransform>()
            .unwrap()
            .last_changed(),
        unchanged
    );

    // Transforms on an ancestor do not turn local geometry into world
    // coordinates, even when the child is temporarily off stage.
    send(
        &mut app,
        child,
        GuiObjectAction::SetPosition(Vec2::new(17., 23.)),
    );
    app.update();
    assert_eq!(
        query
            .get(app.world())
            .unwrap()
            .transform(child)
            .unwrap()
            .position(),
        Vec2::new(17., 23.)
    );
}

#[test]
fn ecs_geometry_tracks_indirect_group_member_movement() {
    let (mut app, asset) = app();
    let package = FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-demo/assets/ui/Basics.fui"),
    )
    .unwrap();
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(
            asset.id(),
            FuiAsset {
                package,
                res_key: "ui/Basics".into(),
            },
        )
        .unwrap();
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "Demo_Controller"))
        .id();
    app.update();
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    let (group, member, old_group, old_member) = {
        let gui = query.get(app.world()).unwrap();
        let group = gui.child_by_name(owner, "n16").unwrap();
        let member = gui.child_by_name(owner, "n13").unwrap();
        (
            group,
            member,
            *gui.transform(group).unwrap(),
            *gui.transform(member).unwrap(),
        )
    };
    let delta = Vec2::new(37., -21.);
    send(
        &mut app,
        group,
        GuiObjectAction::SetPosition(old_group.position() + delta),
    );
    app.update();
    let gui = query.get(app.world()).unwrap();
    assert_eq!(
        gui.transform(member).unwrap().position(),
        old_member.position() + delta
    );
    assert_eq!(gui.get(member).unwrap().parent(), gui.view(owner));
    assert_eq!(gui.transform(member).unwrap().size(), old_member.size());
    let node = app.world().get::<Node>(member).unwrap();
    assert_eq!(node.left, Val::Px(old_member.position().x + delta.x));
    assert_eq!(node.top, Val::Px(old_member.position().y + delta.y));
}

#[test]
fn ecs_roots_capture_global_font_before_their_first_views_are_spawned() {
    use bevy::text::FontSource;
    #[derive(Resource, Default)]
    struct InitialFonts(Vec<FontSource>);
    fn capture(fonts: Query<&TextFont, Added<GObjectRef>>, mut captured: ResMut<InitialFonts>) {
        captured.0 = fonts.iter().map(|font| font.font.clone()).collect();
    }
    let (mut app, asset) = app();
    load(&mut app, &asset);
    app.init_resource::<InitialFonts>()
        .add_systems(Update, capture.in_set(crate::GuiSystems::Initialize));
    let first = app.world().resource::<Assets<Font>>().reserve_handle();
    let second = app.world().resource::<Assets<Font>>().reserve_handle();
    for font in [Some(first), Some(second), None] {
        app.world_mut().resource_mut::<GuiFont>().0 = font.clone();
        app.world_mut()
            .spawn(FairyGuiRoot::new(asset.clone(), "Main"));
        app.update();
        let fonts = &app.world().resource::<InitialFonts>().0;
        assert!(!fonts.is_empty());
        let expected = font.map(FontSource::Handle).unwrap_or_default();
        assert!(fonts.iter().all(|font| *font == expected));
    }
}

#[test]
fn ecs_text_is_current_before_observers_and_their_commands_sync_in_the_same_frame() {
    use fairygui_core::object::EventKind;
    #[derive(Component)]
    struct CopyText {
        input: Entity,
        output: Entity,
    }
    fn copy(event: On<GuiEvent>, gui: GuiQuery, sources: Query<&CopyText>, mut commands: Commands) {
        if event.event().event.kind != EventKind::Click {
            return;
        }
        let source = sources.get(event.entity).unwrap();
        assert!(!gui.display(source.input).unwrap().touchable());
        commands.write_message(GuiObjectCommand::new(
            source.output,
            GuiObjectAction::SetText(gui.text(source.input).unwrap().to_owned()),
        ));
    }
    let (mut app, asset) = app();
    load(&mut app, &asset);
    let owner = app.world_mut().spawn(FairyGuiRoot::new(asset, "Main")).id();
    app.update();
    let root = app.world().get::<GuiRootInstance>(owner).unwrap().object;
    let (input_id, output_id, button_id) = {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        let input = runtime
            .store_mut()
            .create(fairygui_core::package::ObjectType::InputText);
        runtime.store_mut().set_object_text(input, "Old");
        runtime.store_mut().set_input_password(input, true);
        runtime.store_mut().set_input_prompt(input, Some("Prompt"));
        runtime.store_mut().set_parent(input, Some(root));
        let output = runtime
            .store_mut()
            .create(fairygui_core::package::ObjectType::Text);
        runtime.store_mut().set_parent(output, Some(root));
        runtime.queue_subtree(input);
        runtime.queue_subtree(output);
        let button = runtime.store().child_by_name(root, "n1").unwrap();
        (input, output, button)
    };
    app.update();
    let (input, output, button) = {
        let runtime = app.world().non_send::<GuiRuntime>();
        (
            runtime.entity_of(input_id).unwrap(),
            runtime.entity_of(output_id).unwrap(),
            runtime.entity_of(button_id).unwrap(),
        )
    };
    app.world_mut()
        .entity_mut(button)
        .insert(CopyText { input, output })
        .observe(copy);
    // These model edits represent the state left by Input in this frame. The
    // observer must not see the preceding Sync's "Old" snapshot or password mask.
    {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        runtime
            .store_mut()
            .set_object_text(input_id, "本帧😀[b]raw[/b]");
        runtime.store_mut().set_touchable(input_id, false);
        runtime.dispatch(EventKind::Click, button_id);
    }
    app.update();
    assert_eq!(
        app.world().get::<GuiText>(output).unwrap().value(),
        "本帧😀[b]raw[/b]"
    );
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .object_text(output_id),
        Some("本帧😀[b]raw[/b]")
    );

    let changed = app
        .world()
        .entity(input)
        .get_ref::<GuiText>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .entity(input)
            .get_ref::<GuiText>()
            .unwrap()
            .last_changed(),
        changed
    );
    send(&mut app, input, GuiObjectAction::SetText(String::new()));
    app.update();
    assert_eq!(app.world().get::<GuiText>(input).unwrap().value(), "");
}

#[test]
fn ecs_display_keeps_visibility_stage_alpha_and_input_policy_independent() {
    let (mut app, asset) = app();
    load(&mut app, &asset);
    let owner = app.world_mut().spawn(FairyGuiRoot::new(asset, "Main")).id();
    app.update();
    let group_id = {
        let root = app.world().get::<GuiRootInstance>(owner).unwrap().object;
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        let group = runtime
            .store_mut()
            .create(fairygui_core::package::ObjectType::Group);
        runtime.store_mut().set_parent(group, Some(root));
        let button = runtime.store().child_by_name(root, "n1").unwrap();
        runtime.store_mut().set_group(button, Some(group));
        runtime.queue_subtree(group);
        group
    };
    app.update();
    let group = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(group_id)
        .unwrap();
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    let (view, button) = {
        let gui = query.get(app.world()).unwrap();
        (
            gui.view(owner).unwrap(),
            gui.child_by_name(owner, "n1").unwrap(),
        )
    };
    let title = query
        .get(app.world())
        .unwrap()
        .child_by_name(button, "title")
        .unwrap();
    assert!(query.get(app.world()).unwrap().text(owner).is_none());
    for (target, action) in [
        (owner, GuiObjectAction::SetAlpha(0.5)),
        (button, GuiObjectAction::SetAlpha(0.4)),
        (button, GuiObjectAction::SetTouchable(false)),
        (button, GuiObjectAction::SetGrayed(true)),
        (owner, GuiObjectAction::SetVisible(false)),
    ] {
        send(&mut app, target, action);
    }
    app.update();
    {
        let gui = query.get(app.world()).unwrap();
        let root = gui.display(owner).unwrap();
        assert_eq!(gui.display(view), Some(root));
        assert!(!root.visible() && !root.display_visible());
        assert!(
            root.on_stage(),
            "explicit visibility does not detach the stage tree"
        );
        let state = gui.display(button).unwrap();
        assert!(state.visible() && state.on_stage());
        assert!(
            state.display_visible(),
            "ancestor explicit hiding is not local visibility"
        );
        assert_eq!(state.alpha(), 0.4);
        assert_eq!(state.effective_alpha(), 0.2);
        assert!(state.grayed());
        assert!(!state.touchable());
        assert_eq!(gui.display(title).unwrap().effective_alpha(), 0.2);
    }
    send(&mut app, group, GuiObjectAction::SetVisible(false));
    app.update();
    {
        let gui = query.get(app.world()).unwrap();
        let state = gui.display(button).unwrap();
        assert!(state.visible());
        assert!(
            !state.display_visible(),
            "sibling groups affect local drawing"
        );
        assert!(
            state.on_stage(),
            "explicit group hiding does not disconnect display gears"
        );
    }
    app.world_mut()
        .get_mut::<FairyGuiRoot>(owner)
        .unwrap()
        .active = false;
    app.update();
    assert!(
        !query
            .get(app.world())
            .unwrap()
            .display(button)
            .unwrap()
            .on_stage()
    );
    assert!(
        query
            .get(app.world())
            .unwrap()
            .display(button)
            .unwrap()
            .visible()
    );
    let changed = app
        .world()
        .entity(button)
        .get_ref::<GuiDisplay>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .entity(button)
            .get_ref::<GuiDisplay>()
            .unwrap()
            .last_changed(),
        changed
    );
    app.world_mut()
        .get_mut::<FairyGuiRoot>(owner)
        .unwrap()
        .active = true;
    send(&mut app, owner, GuiObjectAction::SetVisible(true));
    send(&mut app, button, GuiObjectAction::SetTouchable(true));
    send(&mut app, button, GuiObjectAction::SetGrayed(false));
    app.update();
    let gui = query.get(app.world()).unwrap();
    assert!(gui.display(button).unwrap().on_stage());
    assert!(gui.display(button).unwrap().touchable());
    assert!(!gui.display(button).unwrap().grayed());
    app.world_mut().despawn(owner);
    app.update();
    let gui = query.get(app.world()).unwrap();
    assert!(gui.display(view).is_none());
    assert!(gui.text(button).is_none());
}
