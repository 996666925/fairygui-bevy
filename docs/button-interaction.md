# Button effects and linked popups

The core model owns button effects. Rendered transforms and colors follow the
ordinary Bevy synchronization systems, so applications can inspect the actual
pressed scale and color through `GuiRuntime::store()`.

## Effects and pointer state

`set_button_down_effect(button, effect, value)` configures an effect at runtime;
exported resources already carry the same configuration. Effect 1 writes an
absolute gray to each non-text direct child supporting the Color property and
restores white afterward. It excludes direct Text/RichText/InputText children;
Label/Button children forward Color to their title. Plain components have no
Color property, so their descendants are not recursively tinted.

Effect 2 multiplies the current scale on entry and divides it on exit, once per
transition. Setting a scale while held therefore affects the eventual restored
scale, matching native behavior. The center pivot retains `pivotAsAnchor`.
Active states are Down, SelectedOver and SelectedDisabled. Invalid nonfinite or
nonpositive runtime scale factors are rejected rather than producing NaNs.

Input uses separate hovered/pressed flags. Leaving or reentering while held does
not release the effect. Common buttons respond to mouse-down/up; Check/Radio
buttons retain their current selection visual while held. A selection change
uses the same visual-state path. Hover handlers require an `over` controller page,
as in GButton. Effects still work when there is no controller. Focus/cursor loss,
hidden/offstage targets and normal capture cancellation release Common effects.

## Linked objects and windows

`linkedPopup` is a runtime object reference in FairyGUI, not an exported URL. Bind
an existing retained object with `set_button_linked_popup(button, Some(popup))`.
For an object built using `create_root`, first call `set_root_attached(index, false)`
to keep it offstage until opened; its root record retains package ownership.

The button opens its link on pointer-down. Before toggling, input checks the
generic popup stack. Clicking inside a popup preserves it and closes only the
popups above it; clicking outside closes all. A popup closed by the current press
cannot immediately reopen during that same press. The next press can reopen it.
Hiding detaches the popup without destroying the model or ECS subtree. Linked
windows use their existing show/hide APIs and retain modal configuration.

`GuiRuntime::show_popup(popup, at)` and `hide_popup(Some(popup))` expose retained
generic popup control; `hide_popup(None)` closes all, and `popup_stack()` inspects
the ordered generic stack. Create the root with `ensure_root(viewport)` before
calling `show_popup` directly. `at` uses root-local coordinates.

Automatic button placement converts the computed target origin into root-local
coordinates and follows GRoot's Auto flip/right-edge rules. The internal root
itself is transparent to pointer targeting (`GRoot.opaque=false`). A top modal
window and popups above it can receive input; layers below the modal cannot.
Pending roots adopted as popups wait for their parent view and preserve their
asset package instead of reattaching as standalone stage roots.

## Evidence and validation

The unchanged official `GButton` class is extracted from
`.refs/FairyGUI-layabox/source/bin/fairygui.js`. Its SHA256 is
`d843237d6e2e4d74865502b3aebf12dfa0fec0cff7c14b74924f03a5f3970f08`.
Rust tests consume the checked-in fixture directly.

The reference runs under a minimal object/event host, including a live display
object so mouse-up does not take the native disposal early return. It covers
139 commands across 12 cases: color/scale effects, repeated states, no controller,
Common/Check/Radio hover and hold, selection/disabled states, and scale changes
while pressed. Core tests compare scale, pressed/hover flags, controller pages and
image/direct-text colors after every command. This does not compare rendered
pixels, gears or asynchronous `fireClick` timers against Laya.

Additional regressions cover real resource-root construction, label title color,
invalid effect factors, focus cancellation, linked windows, nested retained
popups, native positioning, pending-root adoption, modal layering and the internal
root rectangle intercepting input. Real Windows smoke verifies the exported
Button5 center pivot and pressed scale, leaving while held, release restoration,
pressed tint/white restoration and repeated linked-popup open/close on retained
entities. Captures are under `target/fairygui-demo-button-*.png`.

## Programmatic fireClick

`GuiRuntime::fire_button_click(button, down_effect)` implements the native
GButton method. It returns true for an accepted button, false for a missing object
or a different control type. The core `ObjectStore` method returns `Option<bool>`:
None for an invalid target, otherwise whether selection changed.

The call immediately executes internal click behavior: queues its sound, toggles
Check selection, selects an unselected Radio button, or updates a Common button's
related controller. The change-state-on-click setting remains effective.
Selection changes bubble a core StateChanged event with no pointer position.
Programmatic calls work for hidden, offstage and grayed buttons.

Native fireClick invokes `__click` directly rather than dispatching a Click event.
Accordingly it does not emit a GuiEvent/Click, invoke entity observers, open a
linked popup/window, select a menu/list row, check the popup stack, or synthesize
mouse-down/up and pointer capture. Applications wanting a button's internal
behavior can use this method; applications invoking business actions directly
should invoke their own action handler.

With down_effect=true, a Common button enters Over immediately, Down at 100 ms
and Up at 200 ms. Other modes and down_effect=false do not schedule feedback.
Timers call visual-state changes directly, preserving hover/pressed flags and
ignoring later grayed/hover/stage state. Repeated calls append independent timers
(`coverBefore=false`); due callbacks run in registration order within a frame.
Hidden or detached retained buttons continue their feedback. Disposing a subtree
cancels its queued callbacks, and reused arena slots cannot receive them.

The core exposes `advance_button_feedback(delta_seconds: f64)`. Bevy advances it
at the start of Input, before pointer routing and Publish, even without an input
window. Responses calling fire_button_click receive immediate model changes and
same-frame view synchronization. Nonfinite/negative deltas are ignored.

The separate `button-fire-reference.txt` fixture records the same unchanged
GButton class under a deterministic timer/event host. Its 110 commands across 14 cases
cover default/disabled feedback, Common/Check/Radio, sound and StateChanged counts,
the absence of Click, repeated/noncoalesced timers, coarse advances, controllerless
buttons and pointer/disabled interleaving. The host supplies explicit timer
advances in registration order; it is not a capture of Laya's frame scheduler.

Core tests also cover disposal, invalid deltas and related-controller timing.
Bevy integration checks immediate StateChanged without Click and feedback without
an input window. Windows smoke invokes the API in Respond, verifies actual view
scale/restoration and feedback-off behavior, and checks that neither GuiEvent nor
a linked popup was produced. Capture: `target/fairygui-demo-button-fire-down.png`.

Menus, combos and ordinary popups share the retained stack in `popup-lifecycle.md`.
No complete native button/popup pixel parity is claimed.
