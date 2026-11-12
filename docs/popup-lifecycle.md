# Popup stack, retained widgets and resource ownership

`GuiRuntime::popup_stack()` contains ordinary popups, combo dropdowns and the
runtime menu in drawing order. Windows keep their separate window/modal API.
`show_popup`, `hide_popup` and pointer-down checks operate on this shared stack.
The internal root receives `GlobalZIndex(1)` by default, so its window/popup
subtree remains above standalone pages created later. An explicit application
override is preserved. Its own full-window rectangle remains excluded from input.

Pointer-down checks the raw GUI target before a control opens its popup. A hit
inside a stack entry preserves that entry and closes the entries above it. An
outside hit closes the whole stack. Popup IDs closed by that press are remembered,
so a linked button or combo cannot reopen its just-closed content during the same
press. A later press can reopen it.

Combos open on pointer-down, excluding an editable input child, and stay open on
release over the header. Their button controller stays on `down` while open and
returns to `over` or `up` when the popup closes. Clicking a real dropdown row closes
the entire popup stack, forces selection through -1 and publishes StateChanged
with the pointer payload, including when the same row is selected again. Clicking
blank dropdown space does not select or close it.
Initial selection, runtime selection and custom title text update the real
`title` child. Icon selection and cancellation update/clear the `icon` child.

Each combo retains its dropdown, list and item subtree between openings. Values,
icons and captions populate the real list; the visible row count controls height,
and the dropdown follows the combo width. Changing the item data refills through
the object pool. Closing preserves the complete model/ECS subtree; disposing the
combo reclaims its cached dropdown during synchronization.

The runtime menu also retains its content and real child named `list`. Its items
are children of that list, rather than siblings placed directly in the frame.
The caption-based API remains supported. `show_popup_menu_items` accepts core
`MenuItem` definitions and `PopupMenuTemplates` local resource names or `ui://`
URLs. A named item can
carry an optional application action ID and icon. Checkable items use the
template's `checked` controller: 0 is disabled, 1 unchecked, 2 checked.

A row activation waits 100 ms, toggles that controller, hides the menu and entries
above it, then queues one `GuiMenuAction`. `GuiSystems::Publish` publishes it before
`Respond`, where applications may modify objects or reopen the menu in the same
frame. Its payload includes menu/item IDs, stable name, current caption, action ID,
resulting checked state and the original click position. Ordinary `GuiEvent`
observers retain their immediate timing. Disabled, hidden, detached and non-button
rows cannot activate; disabled rows clear the highlight and leave the menu open.
Removing, replacing, clearing, hiding or reopening cancels the pending activation,
including when a removed object's ID is subsequently reused from the pool.

`popup_menu_item(name)` reads current data and its retained object ID.
`update_popup_menu_item(name, |item| ...)` edits caption, icon, action, enabled,
visible and checked/checkable state. Names and separator kinds remain immutable;
such edits return false without changing the menu. A true `checked` value also
enables checkability; to disable it, clear both fields. `add_popup_menu_item`,
`remove_popup_menu_item` and `clear_popup_menu_items` manage rows through pools
keyed by each row's actual template URL. `reopen_popup_menu` shows current data.
`dispose_popup_menu` removes the retained menu and its views; external deletion
of the content/list is detected during synchronization and clears open state.
Showing the same initial definitions again preserves dynamic edits and row IDs;
passing changed definitions replaces the rows. Nonempty names must be unique,
and action rows require a nonempty name.

Separators require `PopupMenuTemplates::separator`. Missing resources reject the
show request while preserving an existing menu. Separators never publish actions,
even if the supplied resource is a button. Their geometry and appearance come
from the resource. The bundled Basics package has no dedicated horizontal menu
separator; tests use its existing image resource to verify mixed template pooling.

Showing clears list selection and fits the frame with a 10-pixel minimum list
height. Dynamic visibility reflows the list when `foldInvisibleItems` is enabled;
it does not refit the frame until the next show, matching native resizeToFit on
display. Core `set_list_fold_invisible_items` also supports tree lists, preserving
logical indices for selection and indentation. Hiding all rows clears scroll
extent.

```rust,ignore
use fairygui_bevy::prelude::*;
use fairygui_bevy::core::{geom::Point, popup::MenuItem};

let items = [MenuItem::action("save", "Save", 42)];
runtime.show_popup_menu_items(
    &mut commands, &package,
    PopupMenuTemplates { content: "PopupMenu", item: "PopupMenuItem", separator: None },
    &items, Point::new(100.0, 100.0),
);
// An application system in GuiSystems::Respond reads MessageReader<GuiMenuAction>.
```

The existing `close_popup_menu` and `close_combo_dropdown` signatures retain their
Commands parameter for compatibility, but closing now detaches retained content.
`hide_popup(None)` closes all popup types. `hide_popup(Some(id))` closes that entry
and everything above it, updating combo/menu bookkeeping as well as stage state.

Resource ownership follows registered source handles, independently of parenting.
Root construction records its objects' source assets, and pending view generation
preserves explicit source boundaries inside a tree. Each recursive render call
resolves the current object's source. Thus an internal GRoot associated with the
first MainMenu package can create a Basics dropdown in the same frame without
borrowing MainMenu atlas IDs. Reparenting a combo preserves its package as well.
Menus retain their registered package; dynamic source handles remain available
while their retained widgets exist.

The native evidence is FairyGUI-layabox `GRoot.ts` showPopup/checkPopups/togglePopup,
`GComboBox.ts` showDropdown/mouse handlers/item callbacks and `PopupMenu.ts` item
timing. Source references are in `.refs/FairyGUI-layabox/source/src/fairygui/`.
Native GRoot placement rules are used for the runtime Auto/Up/Down paths. The
core's older standalone `popup_position` convenience helper remains separate.

Validation covers retained item/model/entity IDs, a mixed ordinary/combo/menu
stack, same-press close suppression, selection and reselection events, delayed
disabled-menu handling, combo-owner disposal, exact background asset paths and
first-popup creation before the internal root view exists. Real Windows smoke
clicks the official Basics combo, selects and reselects a row, opens a menu above
the retained dropdown and verifies closing that mixed stack. Captures include
`target/fairygui-demo-combo-dropdown.png` and `target/fairygui-demo-popup-mixed.png`.

Menu regressions also cover close-before-Respond publication with exact action,
name, pointer and checked payloads, immediate raw clicks, invalidation during the
100 ms delay, disabled/hidden/detached/recycled rows, mixed-template pooling,
dynamic captions/icons, preserved IDs and disposal. The real Basics/Popup page
consumes menu actions to update its button title. Windows smoke opens it through
that button, checks a disabled row, hides/reflows another row, edits a caption,
activates the checkable row and reopens/toggles it again on the same entity. It
also asserts the hovered row to detect a later page occluding the overlay root.
Capture: `target/fairygui-demo-menu-actions.png`.

These checks establish lifecycle, input and rendered resource behavior. They do
not establish full native pixel parity or exact Laya callLater scheduling. The
menu adapter publishes action IDs through Bevy messages rather than storing native
JavaScript callback objects. It supports one pending activation per menu.
Dropdown, default row and menu content/item/separator templates resolve through
registered packages, using both `ui://Package/Item` and canonical ID URLs. Names
and URLs normalize to one canonical cache/pool identity. Each resource keeps its
own source asset handle, including foreign children nested inside a template.
Register dependency packages with `register_package` before opening. Missing
popup/row templates return `None` without replacing a usable menu/dropdown;
registering the missing package permits retry. Retained widgets hold their source
handles. Reload/unregister policies remain application-owned.
Button `fireClick` internal actions and feedback timers are described in
`button-interaction.md`. Other README runtime limits remain open.
