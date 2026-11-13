use crate::prelude::*;
use bevy::{
    ecs::system::SystemState,
    prelude::*,
    text::{EditableText, TextEdit, TextReadWriteMode},
};

fn fixture() -> (App, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .init_resource::<bevy::text::FontCx>()
    .init_resource::<bevy::text::LayoutCx>()
    .init_resource::<bevy::clipboard::Clipboard>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: crate::core::FuiPackage::parse(
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
    let input = SystemState::<GuiQuery>::new(app.world_mut())
        .get(app.world())
        .unwrap()
        .child_by_name(owner, "n22")
        .unwrap();
    (app, owner, input)
}
fn value(app: &App, input: Entity) -> String {
    app.world()
        .get::<EditableText>(input)
        .unwrap()
        .value()
        .to_string()
}
fn edit(app: &mut App, input: Entity, edits: impl IntoIterator<Item = TextEdit>) {
    app.world_mut()
        .get_mut::<EditableText>(input)
        .unwrap()
        .pending_edits
        .extend(edits);
    app.update();
    app.update();
}
fn prompt(app: &mut App, input: Entity) -> Option<Entity> {
    let object = app.world().get::<GObjectRef>(input).unwrap().0;
    app.world_mut()
        .query::<(Entity, &crate::runtime::InputPromptOwner)>()
        .iter(app.world())
        .find_map(|(entity, owner)| (owner.0 == object).then_some(entity))
}

#[test]
fn initialize_input_settings_project_hint_color_password_and_readonly_without_changing_value() {
    #[derive(Resource, Default)]
    struct Seen(bool);
    #[derive(Component)]
    struct FreshInputPage;
    fn initialize(
        roots: Query<Entity, (Added<GuiRootView>, With<FreshInputPage>)>,
        mut ui: ParamSet<(GuiQuery, Query<&mut GuiTextInputSettings>)>,
        mut seen: ResMut<Seen>,
    ) {
        for owner in &roots {
            let input = ui.p0().child_by_name(owner, "n22").unwrap();
            let mut settings = ui.p1();
            let mut settings = settings.get_mut(input).unwrap();
            settings.password = true;
            settings.editable = false;
            settings.prompt_text = Some("[b]PIN code[/b]".into());
            settings.prompt_color = Color::srgb(0.2, 0.6, 1.);
            settings.max_length = 4;
            settings.try_set_restrict(Some("0-9")).unwrap();
            seen.0 = true;
        }
    }
    let (mut app, owner, _) = fixture();
    let asset = app
        .world()
        .get::<FairyGuiRoot>(owner)
        .unwrap()
        .asset
        .clone();
    app.init_resource::<Seen>()
        .add_systems(Update, initialize.in_set(GuiSystems::Initialize));
    let owner = app
        .world_mut()
        .spawn((FairyGuiRoot::new(asset, "Demo_Text"), FreshInputPage))
        .id();
    app.update();
    assert!(app.world().resource::<Seen>().0);
    let input = SystemState::<GuiQuery>::new(app.world_mut())
        .get(app.world())
        .unwrap()
        .child_by_name(owner, "n22")
        .unwrap();
    assert_eq!(value(&app, input), "");
    assert_eq!(app.world().get::<GuiText>(input).unwrap().value(), "");
    assert_eq!(
        app.world().get::<TextReadWriteMode>(input),
        Some(&TextReadWriteMode::ReadOnly)
    );
    assert!(
        app.world()
            .get::<crate::runtime::InputPasswordView>(input)
            .is_some()
    );
    let hint = prompt(&mut app, input).unwrap();
    assert_eq!(app.world().get::<Text>(hint).unwrap().0, "PIN code");
    assert_eq!(
        app.world().get::<TextColor>(hint).unwrap().0.to_srgba(),
        Color::srgb(0.2, 0.6, 1.).to_srgba()
    );
    app.world_mut()
        .get_mut::<GuiTextInputSettings>(input)
        .unwrap()
        .prompt_text = None;
    app.update();
    assert!(prompt(&mut app, input).is_none());
    assert!(app.world().get_entity(hint).is_err());
    assert_eq!(value(&app, input), "");
}

#[test]
fn settings_filter_user_insertions_by_utf16_but_programmatic_values_bypass_limits() {
    let (mut app, _, input) = fixture();
    {
        let mut settings = app
            .world_mut()
            .get_mut::<GuiTextInputSettings>(input)
            .unwrap();
        settings.max_length = 6;
        settings.try_set_restrict(Some("0-9")).unwrap();
    }
    edit(&mut app, input, [TextEdit::Insert("a12b34567".into())]);
    assert_eq!(value(&app, input), "1234");
    assert_eq!(app.world().get::<GuiText>(input).unwrap().value(), "1234");
    {
        let mut settings = app
            .world_mut()
            .get_mut::<GuiTextInputSettings>(input)
            .unwrap();
        settings.max_length = 7;
        settings.try_set_restrict(None).unwrap();
    }
    edit(
        &mut app,
        input,
        [TextEdit::SelectAll, TextEdit::Insert("abc😀123456".into())],
    );
    assert_eq!(value(&app, input), "abc😀12");
    app.world_mut()
        .get_mut::<GuiText>(input)
        .unwrap()
        .set("Program😀12345");
    app.update();
    assert_eq!(value(&app, input), "Program😀12345");
    app.world_mut()
        .get_mut::<GuiTextInputSettings>(input)
        .unwrap()
        .max_length = 1;
    edit(
        &mut app,
        input,
        [
            TextEdit::SelectAll,
            TextEdit::ImeCommit {
                value: "😀1".into(),
            },
        ],
    );
    assert_eq!(value(&app, input), "");
}

#[test]
fn readonly_changes_block_pending_edits_and_password_preserves_real_values_and_identity() {
    let (mut app, _, input) = fixture();
    app.world_mut()
        .get_mut::<GuiText>(input)
        .unwrap()
        .set("base");
    app.update();
    let child = app
        .world_mut()
        .spawn((Node::default(), ChildOf(input)))
        .id();
    app.world_mut()
        .get_mut::<GuiTextInputSettings>(input)
        .unwrap()
        .editable = false;
    edit(
        &mut app,
        input,
        [TextEdit::SelectAll, TextEdit::Insert("blocked".into())],
    );
    assert_eq!(value(&app, input), "base");
    assert_eq!(
        app.world().get::<TextReadWriteMode>(input),
        Some(&TextReadWriteMode::ReadOnly)
    );
    app.world_mut()
        .get_mut::<GuiText>(input)
        .unwrap()
        .set("Programmatic😀");
    app.world_mut()
        .get_mut::<GuiTextInputSettings>(input)
        .unwrap()
        .password = true;
    app.update();
    assert_eq!(value(&app, input), "Programmatic😀");
    assert_eq!(
        app.world().get::<GuiText>(input).unwrap().value(),
        "Programmatic😀"
    );
    assert!(
        app.world()
            .get::<crate::runtime::InputPasswordView>(input)
            .is_some()
    );
    assert!(
        !app.world()
            .get::<EditableText>(input)
            .unwrap()
            .allow_newlines
    );
    app.world_mut()
        .get_mut::<GuiTextInputSettings>(input)
        .unwrap()
        .editable = true;
    app.world_mut()
        .get_mut::<GuiTextInputSettings>(input)
        .unwrap()
        .max_length = 5;
    edit(
        &mut app,
        input,
        [
            TextEdit::SelectAll,
            TextEdit::ImeCommit {
                value: "中😀abc".into(),
            },
        ],
    );
    assert_eq!(value(&app, input), "中😀ab");
    app.world_mut()
        .get_mut::<GuiTextInputSettings>(input)
        .unwrap()
        .password = false;
    app.update();
    assert_eq!(value(&app, input), "中😀ab");
    assert!(
        app.world()
            .get::<crate::runtime::InputPasswordView>(input)
            .is_none()
    );
    assert!(app.world().get_entity(child).is_ok());
}

#[test]
fn invalid_restrictions_keep_the_valid_rule_and_do_not_block_other_field_updates() {
    let (mut app, _, input) = fixture();
    app.world_mut()
        .get_mut::<GuiTextInputSettings>(input)
        .unwrap()
        .try_set_restrict(Some("0-9"))
        .unwrap();
    app.update();
    {
        let mut settings = app
            .world_mut()
            .get_mut::<GuiTextInputSettings>(input)
            .unwrap();
        assert!(settings.try_set_restrict(Some("z-a")).is_err());
        assert_eq!(settings.restrict.as_deref(), Some("0-9"));
        settings.restrict = Some("z-a".into());
        settings.password = true;
        settings.max_length = 4;
        settings.prompt_color = Color::srgba(f32::NAN, 0., 0., 1.);
    }
    app.update();
    let settings = app.world().get::<GuiTextInputSettings>(input).unwrap();
    assert_eq!(settings.restrict.as_deref(), Some("0-9"));
    assert_eq!(settings.max_length, 4);
    assert!(settings.password);
    assert_eq!(settings.prompt_color, Color::srgb_u8(169, 169, 169));
    edit(&mut app, input, [TextEdit::Insert("a19".into())]);
    assert_eq!(value(&app, input), "19");
}

#[test]
fn compound_title_settings_merge_native_changes_and_reclaim_owned_input_views() {
    let (mut app, owner, input) = fixture();
    let input_id = app.world().get::<GObjectRef>(input).unwrap().0;
    let root = app.world().get::<GuiRootInstance>(owner).unwrap().object;
    let label = {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        let label = runtime
            .store_mut()
            .create(crate::core::package::ObjectType::Label);
        runtime.store_mut().set_parent(label, Some(root));
        runtime.store_mut().set_name(input_id, "title");
        runtime.store_mut().set_parent(input_id, Some(label));
        runtime.queue_subtree(label);
        label
    };
    app.update();
    let label_view = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(label)
        .unwrap();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_input_max_length(label, 9);
    app.world_mut()
        .get_mut::<GuiTextInputSettings>(input)
        .unwrap()
        .password = true;
    app.world_mut()
        .get_mut::<GuiTextInputSettings>(label_view)
        .unwrap()
        .prompt_text = Some("Forwarded".into());
    app.update();
    let settings = app.world().get::<GuiTextInputSettings>(label_view).unwrap();
    assert_eq!(settings.max_length, 9);
    assert!(settings.password);
    assert_eq!(settings.prompt_text.as_deref(), Some("Forwarded"));
    assert_eq!(
        app.world().get::<GuiTextInputSettings>(input),
        Some(settings)
    );
    let configured = GuiTextInputSettings {
        editable: false,
        max_length: 3,
        ..default()
    };
    app.world_mut()
        .gui_mut(label_view)
        .unwrap()
        .set_input_settings(configured.clone());
    app.update();
    assert_eq!(
        app.world().get::<GuiTextInputSettings>(input),
        Some(&configured)
    );
    let tick = app
        .world()
        .entity(input)
        .get_ref::<GuiTextInputSettings>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .entity(input)
            .get_ref::<GuiTextInputSettings>()
            .unwrap()
            .last_changed(),
        tick
    );
    app.world_mut().despawn(owner);
    app.update();
    assert!(app.world().get_entity(input).is_err());
    assert!(app.world().get_entity(label_view).is_err());
}
