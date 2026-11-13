use super::*;
use bevy::ecs::system::SystemState;

fn fixture() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin {
            file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/assets").into(),
            ..default()
        },
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .init_resource::<Assets<Font>>()
    .add_message::<bevy::input::mouse::MouseWheel>()
    .add_systems(Update, bind.in_set(GuiSystems::Initialize))
    .add_systems(Update, follow_bottom.in_set(GuiSystems::PostLayout));
    let font = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(
            include_bytes!("../assets/fonts/NotoSansSC-VF.ttf").to_vec(),
        ));
    app.insert_resource(GuiFont(Some(font)));
    let handle = app.world().resource::<AssetServer>().load("ui/Chat.fui");
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(
            handle.id(),
            FuiAsset {
                package: fairygui_bevy::core::FuiPackage::parse(
                    "Chat",
                    include_bytes!("../assets/ui/Chat.fui"),
                )
                .unwrap(),
                res_key: "ui/Chat".into(),
            },
        )
        .unwrap();
    let page = app
        .world_mut()
        .spawn(FairyGuiRoot::new(handle, "Main"))
        .id();
    app.update();
    app.update();
    (app, page)
}
fn child(app: &mut App, owner: Entity, name: &str) -> Entity {
    let mut state = SystemState::<GuiQuery>::new(app.world_mut());
    state
        .get(app.world())
        .unwrap()
        .child_by_name(owner, name)
        .unwrap()
}
fn click(app: &mut App, entity: Entity) {
    let object = app.world().get::<GObjectRef>(entity).unwrap().0;
    app.world_mut().trigger(GuiEvent {
        entity,
        target: Some(entity),
        event: fairygui_bevy::core::object::UiEvent::new(EventKind::Click, object),
    });
    app.update();
}
fn set_input(app: &mut App, page: Entity, text: &str) {
    let input = app.world().get::<Chat>(page).unwrap().input;
    app.world_mut().write_message(GuiObjectCommand::new(
        input,
        GuiObjectAction::SetText(text.into()),
    ));
    app.update();
}
fn verify_rows(app: &mut App, page: Entity) {
    let chat = app.world().get::<Chat>(page).unwrap();
    let messages = chat.messages.clone();
    let list = chat.list;
    let mut state = SystemState::<GuiQuery>::new(app.world_mut());
    let gui = state.get(app.world()).unwrap();
    let list = gui.list(list).unwrap();
    assert_eq!(list.item_count(), messages.len());
    for &row in list.items() {
        let index = app.world().get::<GuiListRow>(row).unwrap().index();
        let message = &messages[index];
        let name = gui.child_by_name(row, "name");
        assert_eq!(name.is_none(), message.from_me);
        if let Some(name) = name {
            assert_eq!(gui.text(name), Some("FairyGUI"));
        }
        let msg = gui.child_by_name(row, "msg").unwrap();
        assert_eq!(gui.text(msg), Some(markup(&message.text).as_str()));
        let transform = gui.transform(msg).unwrap();
        assert!(transform.size().x <= 663.);
    }
}

#[test]
fn chat_entity_observers_send_enter_select_emoji_cap_history_preserve_scroll_and_dispose_popup() {
    let (mut app, page) = fixture();
    let send = child(&mut app, page, "btnSend1");
    let emoji_button = child(&mut app, page, "btnEmoji1");
    let chat = app.world().get::<Chat>(page).unwrap();
    let (input, list, popup) = (chat.input, chat.list, chat.emoji);
    assert!(chat.messages.is_empty());
    assert!(
        app.world()
            .get::<GuiListState>(list)
            .unwrap()
            .items()
            .is_empty()
    );
    click(&mut app, send);
    assert!(app.world().get::<Chat>(page).unwrap().messages.is_empty());
    set_input(&mut app, page, "Hello 中文 [:cool] [b]bold[/b]");
    click(&mut app, send);
    let messages = &app.world().get::<Chat>(page).unwrap().messages;
    assert_eq!(messages.len(), 2);
    assert!(messages[0].from_me && !messages[1].from_me);
    assert_eq!(messages[1].text, "Today is a good day. ");
    assert_eq!(app.world().get::<GuiText>(input).unwrap().value(), "");
    verify_rows(&mut app, page);
    click(&mut app, emoji_button);
    let popup_view = app.world().get::<GuiRootView>(popup).unwrap().0;
    assert!(app.world().resource::<GuiPopupState>().is_open(popup_view));
    let emoji_list = child(&mut app, popup, "list");
    let item = app.world().get::<GuiListState>(emoji_list).unwrap().items()[5];
    let id = app.world().get::<GObjectRef>(emoji_list).unwrap().0;
    app.world_mut().trigger(GuiListItemClick {
        entity: emoji_list,
        item,
        index: 5,
        shift: false,
        ctrl: false,
        event: fairygui_bevy::core::object::UiEvent::new(EventKind::ClickItem, id),
    });
    app.update();
    assert_eq!(
        app.world().get::<GuiText>(input).unwrap().value(),
        "[:cool]"
    );
    assert!(
        app.world().resource::<GuiPopupState>().is_open(popup_view),
        "native emoji selection retains popup"
    );
    // Enter observes the input directly and uses the notification's committed value.
    let input_id = app.world().get::<GObjectRef>(input).unwrap().0;
    app.world_mut().trigger(GuiTextInput {
        entity: input,
        value: "Enter [:88]".into(),
        event: fairygui_bevy::core::object::UiEvent::new(EventKind::Enter, input_id),
    });
    app.update();
    assert!(
        app.world()
            .get::<Chat>(page)
            .unwrap()
            .messages
            .iter()
            .any(|message| message.text == "Enter [:88]")
    );
    for index in 0..105 {
        app.world_mut().trigger(Submit {
            entity: page,
            text: format!("Message {index}"),
        });
        app.update();
    }
    assert_eq!(app.world().get::<Chat>(page).unwrap().messages.len(), 100);
    assert!(
        !app.world()
            .get::<Chat>(page)
            .unwrap()
            .messages
            .iter()
            .any(|message| message.text.starts_with("Hello"))
    );
    let bottom = *app.world().get::<GuiScrollState>(list).unwrap();
    assert!(bottom.max_position().y > 0.);
    assert_eq!(bottom.position().y, bottom.max_position().y);
    verify_rows(&mut app, page);
    app.world_mut().write_message(GuiObjectCommand::new(
        list,
        GuiObjectAction::ScrollTo {
            position: Vec2::new(0., 120.),
            animated: false,
        },
    ));
    app.update();
    let before = app.world().get::<GuiScrollState>(list).unwrap().position();
    app.world_mut().trigger(Submit {
        entity: page,
        text: "Keep my scroll".into(),
    });
    app.update();
    assert_eq!(
        app.world().get::<GuiScrollState>(list).unwrap().position(),
        before
    );
    verify_rows(&mut app, page);
    app.world_mut().entity_mut(page).despawn();
    app.update();
    assert!(app.world().get_entity(popup).is_err());
    assert!(app.world().get_entity(popup_view).is_err());
    assert!(app.world().resource::<GuiPopupState>().stack().is_empty());
}

#[test]
fn chat_custom_emoji_tags_preserve_native_ubb_styles_unknown_tags_and_escaped_brackets() {
    for (index, tag) in TAGS.iter().enumerate() {
        assert_eq!(
            markup(&format!("[:{}]", tag.to_uppercase())),
            format!("<img src='ui://Chat/1f6{index:02}'/>")
        );
    }
    assert_eq!(
        markup("[b]hi[/b] [:unknown] \\[:cool]"),
        "<b>hi</b> [:unknown] [:cool]"
    );
}
