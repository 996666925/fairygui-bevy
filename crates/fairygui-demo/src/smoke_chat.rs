//! Real menu navigation, keyboard submission, emoji picking and history scrolling.
use super::*;
use bevy::input::{
    ButtonState,
    keyboard::{Key, KeyCode, KeyboardInput},
};

#[derive(Resource, Default)]
struct State {
    step: u8,
    frame: u32,
    since: u32,
    page: Option<Entity>,
    popup: Option<Entity>,
    popup_view: Option<Entity>,
    cursor: Option<Vec2>,
    sent: usize,
    count: usize,
    before: Vec2,
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<State>().add_systems(
        PreUpdate,
        drive
            .after(InputSystems)
            .before(UiSystems::Focus)
            .before(PickingSystems::ProcessInput),
    );
}
fn key(commands: &mut Commands, window: Entity, text: Option<&str>) {
    let text = text.map(str::to_owned);
    commands.queue(move |world: &mut World| {
        world.write_message(KeyboardInput {
            key_code: if text.is_some() {
                KeyCode::KeyH
            } else {
                KeyCode::Enter
            },
            logical_key: text
                .as_ref()
                .map_or(Key::Enter, |text| Key::Character(text.clone().into())),
            text: text.map(Into::into),
            state: ButtonState::Pressed,
            repeat: false,
            window,
        });
    });
}
#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    demo: Res<Demo>,
    gui: GuiQuery,
    chats: Query<&crate::chat_demo::Chat>,
    existing: Query<()>,
    popups: Res<GuiPopupState>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    mut windows: Query<(Entity, &mut Window)>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
    time: Res<Time<bevy::time::Real>>,
    mut gui_edit: GuiMut,
) {
    state.frame += 1;
    assert!(
        time.elapsed_secs_f64() < 120.,
        "chat smoke timed out at step {}",
        state.step
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    let elapsed = state.frame - state.since;
    let center =
        |entity| nodes.get(entity).unwrap().1.transform_point2(Vec2::ZERO) / window.scale_factor();
    let mut next = false;
    match state.step {
        0 => {
            let Some(menu) = demo.menu_owner.filter(|&menu| gui.view(menu).is_some()) else {
                return;
            };
            let Some(button) = gui.child_by_name(menu, "n11") else {
                return;
            };
            if nodes.get(button).is_err() {
                return;
            }
            state.cursor = Some(center(button));
            next = true;
        }
        1 | 4 | 8 | 11 | 14 | 17 | 22 | 28 | 32 | 36 | 40 | 44 if elapsed >= 3 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        2 | 5 | 9 | 12 | 15 | 18 | 23 | 29 | 33 | 37 | 41 | 45 if elapsed >= 3 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        3 if elapsed >= 12 => {
            let Some((page, _, name)) = demo.open.as_ref() else {
                return;
            };
            assert_eq!(name, "Chat");
            let Ok(chat) = chats.get(*page) else { return };
            if gui.view(chat.emoji).is_none() {
                return;
            }
            assert!(chat.messages.is_empty());
            assert!(gui.list(chat.list).unwrap().items().is_empty());
            assert_eq!(gui.transform(*page).unwrap().position(), Vec2::ZERO);
            state.page = Some(*page);
            state.popup = Some(chat.emoji);
            state.popup_view = gui.view(chat.emoji);
            state.cursor = Some(center(chat.input));
            next = true;
        }
        6 if elapsed >= 4 => {
            key(&mut commands, window_entity, Some("Hello 中文 [b]Bevy[/b]"));
            next = true;
        }
        7 if elapsed >= 5 => {
            let chat = chats.get(state.page.unwrap()).unwrap();
            assert_eq!(gui.text(chat.input), Some("Hello 中文 [b]Bevy[/b]"));
            state.cursor = Some(center(
                gui.child_by_name(state.page.unwrap(), "btnSend1").unwrap(),
            ));
            next = true;
        }
        10 if elapsed >= 6 => {
            let chat = chats.get(state.page.unwrap()).unwrap();
            assert_eq!(chat.messages.len(), 2);
            assert!(chat.messages[0].from_me && !chat.messages[1].from_me);
            assert_eq!(gui.text(chat.input), Some(""));
            state.cursor = Some(center(
                gui.child_by_name(state.page.unwrap(), "btnEmoji1").unwrap(),
            ));
            next = true;
        }
        13 if elapsed >= 6 => {
            assert!(popups.is_open(state.popup_view.unwrap()));
            let list = gui.child_by_name(state.popup.unwrap(), "list").unwrap();
            let item = gui.list(list).unwrap().items()[5];
            assert_eq!(gui.text(item), Some("cool"));
            let button = gui.child_by_name(state.page.unwrap(), "btnEmoji1").unwrap();
            let origin = gui.local_to_global(button, Vec2::ZERO).unwrap();
            let popup = gui.transform(state.popup.unwrap()).unwrap();
            assert!((popup.position().y - (origin.y - popup.size().y - 1.)).abs() < 0.1);
            state.cursor = Some(center(item));
            next = true;
        }
        16 if elapsed >= 6 => {
            let chat = chats.get(state.page.unwrap()).unwrap();
            assert_eq!(gui.text(chat.input), Some("[:cool]"));
            assert!(popups.is_open(state.popup_view.unwrap()));
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-chat-emoji.png"));
            state.cursor = Some(center(chat.input));
            next = true;
        }
        19 if elapsed >= 4 => {
            key(&mut commands, window_entity, None);
            next = true;
        }
        20 if elapsed >= 8 => {
            let chat = chats.get(state.page.unwrap()).unwrap();
            assert!(
                chat.messages
                    .iter()
                    .any(|message| message.from_me && message.text == "[:cool]")
            );
            assert_eq!(gui.text(chat.input), Some(""));
            assert!(!popups.is_open(state.popup_view.unwrap()));
            state.cursor = Some(center(
                gui.child_by_name(state.page.unwrap(), "btnSend1").unwrap(),
            ));
            next = true;
        }
        21 if elapsed >= 2 => {
            let chat = chats.get(state.page.unwrap()).unwrap();
            gui_edit.edit(chat.input, |object: &mut GuiObjectMut| {
                object.set_text(format!("History {} [:cool]", state.sent))
            });
            next = true;
        }
        24 if elapsed >= 3 => {
            state.sent += 1;
            let chat = chats.get(state.page.unwrap()).unwrap();
            let scroll = gui.scroll(chat.list).unwrap();
            assert!(
                (scroll.position().y - scroll.max_position().y).abs() < 0.1,
                "follow bottom after sending"
            );
            if state.sent < 105 {
                state.step = 21;
                state.since = state.frame;
            } else {
                assert_eq!(chat.messages.len(), 100);
                gui_edit.edit(chat.list, |object: &mut GuiObjectMut| {
                    object.scroll_to(Vec2::new(0., 120.), false)
                });
                next = true;
            }
        }
        25 if elapsed >= 5 => {
            let chat = chats.get(state.page.unwrap()).unwrap();
            state.before = gui.scroll(chat.list).unwrap().position();
            assert!(state.before.y < gui.scroll(chat.list).unwrap().max_position().y);
            gui_edit.edit(chat.input, |object: &mut GuiObjectMut| {
                object.set_text("Keep my history position")
            });
            state.count = chat.messages.len();
            next = true;
        }
        26 if elapsed >= 4 => {
            next = true;
        }
        27 if elapsed >= 2 => {
            next = true;
        }
        30 if elapsed >= 6 => {
            let chat = chats.get(state.page.unwrap()).unwrap();
            assert_eq!(chat.messages.len(), state.count);
            assert_eq!(gui.scroll(chat.list).unwrap().position(), state.before);
            assert!(
                chat.messages
                    .iter()
                    .any(|message| message.text == "Keep my history position")
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-chat-history.png"));
            state.cursor = Some(center(
                gui.child_by_name(state.page.unwrap(), "btnEmoji1").unwrap(),
            ));
            next = true;
        }
        31 if elapsed >= 3 => {
            next = true;
        }
        34 if elapsed >= 6 => {
            assert!(popups.is_open(state.popup_view.unwrap()));
            assert_eq!(gui.view(state.popup.unwrap()), state.popup_view);
            state.cursor = Some(center(gui.view(demo.close_owner.unwrap()).unwrap()));
            next = true;
        }
        35 if elapsed >= 3 => {
            next = true;
        }
        38 if elapsed >= 8 => {
            assert!(demo.open.is_none());
            assert!(!existing.contains(state.popup.unwrap()));
            assert!(!existing.contains(state.popup_view.unwrap()));
            assert!(popups.stack().is_empty());
            state.cursor = Some(center(
                gui.child_by_name(demo.menu_owner.unwrap(), "n11").unwrap(),
            ));
            next = true;
        }
        39 if elapsed >= 3 => {
            next = true;
        }
        42 if elapsed >= 12 => {
            let page = demo.open.as_ref().unwrap().0;
            assert_ne!(page, state.page.unwrap());
            assert!(chats.get(page).unwrap().messages.is_empty());
            state.cursor = Some(center(gui.view(demo.close_owner.unwrap()).unwrap()));
            next = true;
        }
        43 if elapsed >= 3 => {
            next = true;
        }
        46 if elapsed >= 6 && demo.open.is_none() => {
            println!(
                "demo smoke PASSED: Chat menu navigation, real keyboard/button/Enter submission, first reply, emoji popup/append/inline markup/reuse, 100-message cap, bottom following/history preservation and owned popup disposal"
            );
            exit.write(AppExit::Success);
            next = true;
        }
        _ => {}
    }
    if next {
        state.step += 1;
        state.since = state.frame;
    }
    window.set_cursor_position(state.cursor);
    move_pointer(window_entity, state.cursor, &mut pointers);
}
