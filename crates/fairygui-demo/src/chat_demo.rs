//! Chat state belongs to its page; controls observe only their own events.
use bevy::{prelude::*, window::PrimaryWindow};
use fairygui_bevy::{core::object::EventKind, prelude::*};
use std::{hash::BuildHasher, sync::Arc};

#[cfg(test)]
#[path = "chat_demo_tests.rs"]
mod tests;

const TAGS: [&str; 14] = [
    "88", "am", "bs", "bz", "ch", "cool", "dhq", "dn", "fd", "gz", "han", "hx", "hxiao", "hxiu",
];

#[derive(Clone)]
pub(crate) struct Message {
    pub text: String,
    pub from_me: bool,
}

#[derive(Component)]
pub(crate) struct Chat {
    pub list: Entity,
    pub input: Entity,
    pub emoji: Entity,
    pub messages: Arc<Vec<Message>>,
    random: std::collections::hash_map::RandomState,
    sequence: u64,
    follow_bottom: bool,
}
#[derive(Component)]
struct Owner(Entity);
#[derive(Component)]
pub(crate) struct EmojiOwner(Entity);
#[derive(Component)]
pub(crate) struct EmojiBound;
#[derive(Component)]
pub(crate) struct ChatFixture;
#[derive(EntityEvent)]
struct Submit {
    entity: Entity,
    text: String,
}

/// Expand the demo's custom tags before running the ordinary native UBB parser.
fn markup(text: &str) -> String {
    let mut expanded = String::new();
    let mut at = 0;
    while at < text.len() {
        let rest = &text[at..];
        if rest.starts_with("\\[") {
            expanded.push_str("\\[");
            at += 2;
            continue;
        }
        if let Some(tag) = rest.strip_prefix("[:")
            && let Some(end) = tag.find(']')
            && let Some(index) = TAGS
                .iter()
                .position(|known| known.eq_ignore_ascii_case(&tag[..end]))
        {
            expanded.push_str(&format!("<img src='ui://Chat/1f6{index:02}'/>"));
            at += end + 3;
            continue;
        }
        let ch = rest.chars().next().unwrap();
        expanded.push(ch);
        at += ch.len_utf8();
    }
    fairygui_bevy::core::text::ubb_to_html(&expanded, &default())
}

fn content(messages: Arc<Vec<Message>>) -> GuiVirtualList {
    let provider = messages.clone();
    GuiVirtualList::new(messages.len(), move |index, row| {
        let message = &messages[index];
        if let Some(mut name) = row.child_by_path("name") {
            name.set_text("FairyGUI");
        }
        row.set_icon(if message.from_me {
            "ui://Chat/r0"
        } else {
            "ui://Chat/r1"
        });
        if let Some(mut text) = row.child_by_path("msg") {
            text.set_text(&markup(&message.text));
        }
    })
    .with_item_provider(move |index| {
        Some(
            if provider[index].from_me {
                "chatRight"
            } else {
                "chatLeft"
            }
            .into(),
        )
    })
}

#[allow(clippy::type_complexity)]
pub fn bind(
    mut commands: Commands,
    pages: Query<(Entity, &FairyGuiRoot), (Without<Chat>, Without<ChatFixture>)>,
    popups: Query<(Entity, &EmojiOwner), Without<EmojiBound>>,
    gui: GuiQuery,
) {
    for (page, source) in &pages {
        if source.item_name != "Main"
            || !source
                .asset
                .path()
                .is_some_and(|p| p.path().ends_with("Chat.fui"))
        {
            continue;
        }
        let Some(list) = gui.child_by_name(page, "list") else {
            continue;
        };
        let input = gui.child_by_name(page, "input1").unwrap();
        let send = gui.child_by_name(page, "btnSend1").unwrap();
        let emoji_button = gui.child_by_name(page, "btnEmoji1").unwrap();
        let mut popup = FairyGuiRoot::new(source.asset.clone(), "EmojiSelectUI");
        popup.active = false;
        let emoji = commands
            .spawn((popup, EmojiOwner(page), ChildOf(page)))
            .id();
        let messages = Arc::new(Vec::new());
        commands.entity(list).insert(content(messages.clone()));
        commands.entity(input).insert(Owner(page)).observe(enter);
        commands
            .entity(send)
            .insert(Owner(page))
            .observe(send_clicked);
        commands
            .entity(emoji_button)
            .insert(Owner(page))
            .observe(emoji_clicked);
        commands
            .entity(page)
            .insert(Chat {
                list,
                input,
                emoji,
                messages,
                random: default(),
                sequence: 0,
                follow_bottom: false,
            })
            .observe(submit);
    }
    for (popup, owner) in &popups {
        let Some(list) = gui.child_by_name(popup, "list") else {
            continue;
        };
        commands
            .entity(list)
            .insert(Owner(owner.0))
            .observe(choose_emoji);
        commands.entity(popup).insert(EmojiBound);
    }
}

fn enter(event: On<GuiTextInput>, owners: Query<&Owner>, mut commands: Commands) {
    if event.event.kind == EventKind::Enter
        && let Ok(owner) = owners.get(event.entity)
    {
        commands.trigger(Submit {
            entity: owner.0,
            text: event.value.clone(),
        });
    }
}
fn send_clicked(
    event: On<GuiEvent>,
    owners: Query<&Owner>,
    pages: Query<&Chat>,
    gui: GuiQuery,
    mut commands: Commands,
) {
    if event.event.kind != EventKind::Click {
        return;
    }
    let Ok(owner) = owners.get(event.entity) else {
        return;
    };
    let Ok(page) = pages.get(owner.0) else { return };
    commands.trigger(Submit {
        entity: owner.0,
        text: gui.text(page.input).unwrap_or_default().into(),
    });
}
fn submit(event: On<Submit>, mut pages: Query<&mut Chat>, gui: GuiQuery, mut commands: Commands) {
    if event.text.is_empty() {
        return;
    }
    let Ok(mut chat) = pages.get_mut(event.entity) else {
        return;
    };
    let bottom = gui
        .scroll(chat.list)
        .is_some_and(|scroll| scroll.position().y == scroll.max_position().y);
    chat.sequence += 1;
    let reply = chat.messages.is_empty() || chat.random.hash_one(chat.sequence).is_multiple_of(2);
    let messages = Arc::make_mut(&mut chat.messages);
    messages.push(Message {
        text: event.text.clone(),
        from_me: true,
    });
    if reply {
        messages.push(Message {
            text: "Today is a good day. ".into(),
            from_me: false,
        });
    }
    if messages.len() > 100 {
        messages.drain(..messages.len() - 100);
    }
    chat.follow_bottom = bottom;
    commands
        .entity(chat.list)
        .insert(content(chat.messages.clone()));
    commands.entity(chat.input).insert(GuiText::new(""));
}
fn emoji_clicked(
    event: On<GuiEvent>,
    owners: Query<&Owner>,
    pages: Query<&Chat>,
    mut commands: Commands,
) {
    if event.event.kind != EventKind::Click {
        return;
    }
    let Ok(owner) = owners.get(event.entity) else {
        return;
    };
    let Ok(chat) = pages.get(owner.0) else { return };
    commands.write_message(GuiPopupCommand::ShowAnchored {
        popup: chat.emoji,
        anchor: event.entity,
        direction: GuiPopupDirection::Up,
    });
}
fn choose_emoji(
    event: On<GuiListItemClick>,
    owners: Query<&Owner>,
    pages: Query<&Chat>,
    gui: GuiQuery,
    mut gui_edit: GuiMut,
) {
    let Ok(owner) = owners.get(event.entity) else {
        return;
    };
    let Ok(chat) = pages.get(owner.0) else { return };
    let Some(tag) = gui.text(event.item) else {
        return;
    };
    gui_edit.edit(chat.input, |object: &mut GuiObjectMut| {
        object.set_text(format!(
            "{}[:{tag}]",
            gui.text(chat.input).unwrap_or_default()
        ))
    });
}

pub fn follow_bottom(mut pages: Query<&mut Chat>, mut objects: GuiMut) {
    for mut chat in &mut pages {
        if !chat.follow_bottom {
            continue;
        }
        chat.follow_bottom = false;
        objects.edit(chat.list, |object: &mut GuiObjectMut| {
            object.set_scroll_percentage(Vec2::new(0., 1.), false)
        });
    }
}

pub fn fit(
    pages: Query<Entity, With<Chat>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    gui: GuiQuery,
    mut objects: GuiMut,
) {
    let Ok(window) = windows.single() else { return };
    let size = Vec2::new(window.width(), window.height());
    for page in &pages {
        if gui
            .transform(page)
            .is_some_and(|transform| transform.size() != size)
        {
            objects.edit(page, |object: &mut GuiObjectMut| object.set_size(size));
        }
    }
}
