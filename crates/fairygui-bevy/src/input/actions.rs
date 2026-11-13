//! Built-in widget actions.
use super::*;
pub(super) fn combo_click(
    runtime: &mut GuiRuntime,
    object: GObjectId,
    cursor: Vec2,
    touch_id: Option<u64>,
) -> bool {
    if let Some(combo) = runtime.store().combo_of_popup(object) {
        if let Some(index) = list_child_index(runtime, combo, object) {
            // Native defers the item handler, hides the complete root popup
            // stack, then forces the selected title/icon to reapply on reselect.
            runtime.hide_popup(None);
            runtime
                .store_mut()
                .select_combo_index_from_input(combo, index as i32);
            let mut event = UiEvent::new(EventKind::StateChanged, combo)
                .with_pointer(Point::new(cursor.x, cursor.y));
            event.touch_id = touch_id;
            runtime.dispatch_event(event);
            return true;
        }
        return false;
    }
    // Opening/toggling belongs to mouse-down. A release on the combo consumes
    // no second toggle and does not perform list selection on the combo itself.
    let mut current = Some(object);
    while let Some(id) = current {
        let Some(item) = runtime.store().object(id) else {
            break;
        };
        if matches!(
            item.control(),
            fairygui_core::widget::ControlData::ComboBox(_)
        ) {
            return true;
        }
        current = item.parent();
    }
    false
}

/// 对象在弹层列表里的序号。它可能是项内部的子对象，因此要向上找到列表的直接子项。
fn list_child_index(runtime: &GuiRuntime, combo: GObjectId, object: GObjectId) -> Option<usize> {
    let list = runtime.store().combo_popup(combo)?.list;
    let store = runtime.store();

    let mut current = Some(object);
    while let Some(id) = current {
        let item = store.object(id)?;
        if item.parent() == Some(list) {
            return store.list_item_index(list, id);
        }
        current = item.parent();
    }
    None
}

/// 命中的对象若是某个窗口的关闭按钮，返回那个窗口的编号。
pub(super) fn close_button_window(runtime: &GuiRuntime, object: GObjectId) -> Option<usize> {
    let index = runtime.window_containing(object)?;
    let window = runtime.window(index)?;
    let close = window.close_button()?;
    let mut current = Some(object);
    while let Some(id) = current {
        if id == close {
            return Some(index);
        }
        current = runtime.store().object(id)?.parent();
    }
    None
}
