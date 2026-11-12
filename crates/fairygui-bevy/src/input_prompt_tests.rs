use super::*;
use bevy::text::{EditableText, TextEdit};

fn fixture() -> (App, usize, GObjectId, Entity) {
    let package = FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-core/tests/fixtures/BitmapText.fui"),
    )
    .unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<FuiAsset>()
        .init_asset::<Image>()
        .init_asset::<Font>()
        .init_resource::<GuiFont>()
        .init_resource::<bevy::text::FontCx>()
        .init_resource::<bevy::text::LayoutCx>()
        .init_resource::<bevy::clipboard::Clipboard>();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: package.clone(),
            res_key: "ui/Basics".into(),
        });
    let mut runtime = GuiRuntime::default();
    let index = runtime.create_root(handle, &package, "Demo_Text").unwrap();
    let root = runtime.root_object(index).unwrap();
    let input = runtime.store().child_by_name(root, "n22").unwrap();
    runtime.store_mut().set_object_text(input, "");
    runtime.store_mut().set_object_font(input, None);
    runtime
        .store_mut()
        .set_input_prompt(input, Some("[color=red][b]请输入名字[/b][/color]"));
    runtime.store_mut().set_size(input, 240., 70.);
    app.insert_non_send(runtime)
        .add_systems(
            Update,
            (
                spawn_pending_roots,
                spawn_pending_objects,
                sync_text_inputs,
                sync_dirty,
                sync_text_fonts,
                sync_input_policy,
                sync_bitmap_rich_text,
                sync_rich_text,
                sync_input_prompts,
                sync_appearance,
                sync_stage_hierarchy,
            )
                .chain(),
        )
        .add_systems(
            PostUpdate,
            (apply_input_edits, update_prompt_visibility).chain(),
        );
    app.update();
    app.update();
    let entity = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(input)
        .unwrap();
    (app, index, input, entity)
}
fn prompt(app: &App, owner: Entity) -> Entity {
    app.world().get::<InputPromptView>(owner).unwrap().0
}
fn queue(app: &mut App, owner: Entity, edits: impl IntoIterator<Item = TextEdit>) {
    app.world_mut()
        .get_mut::<EditableText>(owner)
        .unwrap()
        .pending_edits
        .extend(edits);
    app.update();
}

#[test]
fn empty_prompt_keeps_model_empty_and_tracks_commit_preedit_color_alpha_and_stable_ownership() {
    let (mut app, _, owner, entity) = fixture();
    let view = prompt(&app, entity);
    assert_eq!(app.world().get::<Text>(view).unwrap().0, "请输入名字");
    assert_eq!(
        app.world().get::<TextColor>(view).unwrap().0,
        bevy::color::Color::srgb_u8(169, 169, 169)
    );
    assert!(app.world().get::<Text>(entity).unwrap().0.is_empty());
    assert!(
        app.world()
            .get::<EditableText>(entity)
            .unwrap()
            .value()
            .to_string()
            .is_empty()
    );
    assert!(app.world().get::<GObjectRef>(view).is_none());
    assert_eq!(
        app.world().get::<bevy::picking::Pickable>(view),
        Some(&bevy::picking::Pickable::IGNORE)
    );
    assert_eq!(app.world().get::<ChildOf>(view).unwrap().parent(), entity);
    app.insert_resource(bevy::input_focus::InputFocus::from_entity(entity));
    app.update();
    assert_eq!(
        app.world().get::<Visibility>(view),
        Some(&Visibility::Inherited)
    );
    app.world_mut()
        .resource_mut::<bevy::input_focus::InputFocus>()
        .clear();
    app.update();
    assert_eq!(prompt(&app, entity), view);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_alpha(owner, 0.4);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_grayed(owner, true);
    for _ in 0..60 {
        app.update();
    }
    assert_eq!(prompt(&app, entity), view);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_input_prompt_color(
            owner,
            fairygui_core::color::Color::from_rgba8(40, 100, 180, 255),
        );
    app.update();
    let recolored = prompt(&app, entity);
    assert_ne!(recolored, view);
    assert!(app.world().get_entity(view).is_err());
    assert_eq!(
        app.world()
            .get::<appearance::TextTintSource>(recolored)
            .unwrap()
            .color,
        Some(fairygui_core::color::Color::from_rgba8(40, 100, 180, 255))
    );
    let view = recolored;
    let tint = app.world().get::<TextColor>(view).unwrap().0.to_srgba();
    assert!((tint.alpha - 0.4).abs() < 1e-6);
    assert!((tint.red - tint.green).abs() < 1e-6);
    queue(
        &mut app,
        entity,
        [TextEdit::ImeSetCompose {
            value: "预编辑".into(),
            cursor: None,
        }],
    );
    assert_eq!(
        app.world().get::<Visibility>(view),
        Some(&Visibility::Hidden)
    );
    queue(
        &mut app,
        entity,
        [TextEdit::ImeCommit {
            value: "值".into()
        }],
    );
    assert_eq!(
        app.world().get::<Visibility>(view),
        Some(&Visibility::Hidden)
    );
    app.update();
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .object(owner)
            .unwrap()
            .control()
            .as_text_field()
            .unwrap()
            .text,
        "值"
    );
    queue(&mut app, entity, [TextEdit::SelectAll, TextEdit::Backspace]);
    assert_eq!(
        app.world().get::<Visibility>(view),
        Some(&Visibility::Inherited)
    );
    app.update();
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .object(owner)
            .unwrap()
            .control()
            .as_text_field()
            .unwrap()
            .text
            .is_empty()
    );
    assert_eq!(prompt(&app, entity), view);
}

#[test]
fn prompt_translation_bitmap_switch_pooling_and_disposal_reclaim_only_owned_views() {
    let (mut app, index, owner, entity) = fixture();
    let app_child = app
        .world_mut()
        .spawn((Node::default(), ChildOf(entity)))
        .id();
    let old = prompt(&app, entity);
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let mut table = fairygui_core::translation::TranslationTable::new();
        table.set("hint", "Localized prompt");
        gui.set_translations(table);
        gui.store_mut()
            .set_input_prompt(owner, Some("[b]!hint![/b]"));
    }
    app.update();
    let translated = prompt(&app, entity);
    assert!(app.world().get_entity(old).is_err());
    assert_eq!(
        app.world().get::<Text>(translated).unwrap().0,
        "Localized prompt"
    );
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut().set_input_prompt(owner, Some("UI#"));
        gui.store_mut().set_object_font(owner, Some("BMFontTest"));
    }
    app.update();
    app.update();
    let bitmap = prompt(&app, entity);
    assert!(app.world().get_entity(translated).is_err());
    assert!(app.world().get::<Text>(bitmap).is_none());
    let quads: Vec<_> = app
        .world()
        .get::<Children>(bitmap)
        .unwrap()
        .iter()
        .collect();
    assert_eq!(quads.len(), 3);
    assert!(
        quads
            .iter()
            .all(|&e| app.world().get::<ImageNode>(e).is_some())
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_font(owner, Some("HitNumber"));
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_input_prompt(owner, Some("123"));
    app.update();
    let colored = prompt(&app, entity);
    let part = app
        .world()
        .get::<Children>(colored)
        .unwrap()
        .iter()
        .next()
        .unwrap();
    assert_eq!(
        app.world()
            .get::<appearance::TextTintSource>(part)
            .unwrap()
            .color,
        Some(fairygui_core::color::Color::WHITE)
    );
    assert!(quads.iter().all(|&e| app.world().get_entity(e).is_err()));
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let gui = &mut *gui;
        gui.pool.release(&mut gui.store, "prompt", owner);
    }
    app.update();
    assert_eq!(
        app.world().get::<Visibility>(entity),
        Some(&Visibility::Hidden)
    );
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let gui = &mut *gui;
        assert_eq!(gui.pool.acquire_checked(&gui.store, "prompt"), Some(owner));
        let root = gui.root_object(index).unwrap();
        gui.store.set_parent(owner, Some(root));
        gui.store.set_visible(owner, true);
    }
    app.update();
    assert_eq!(prompt(&app, entity), colored);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_input_prompt(owner, None);
    app.update();
    assert!(app.world().get_entity(colored).is_err());
    assert!(app.world().get::<InputPromptView>(entity).is_none());
    assert!(app.world().get_entity(app_child).is_ok());
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_input_prompt(owner, Some("456"));
    app.update();
    let last = prompt(&app, entity);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .remove(owner);
    app.update();
    assert!(app.world().get_entity(entity).is_err());
    assert!(app.world().get_entity(last).is_err());
}
