# Runtime gear state

Property edits now update the active gear page where FairyGUI-Laya's setter
calls updateGear. Returning to a page restores the edited values instead of the
original exported values. Bevy remains on 0.20.1.

## Capture rules

| Gear | Native setter behavior implemented |
| --- | --- |
| XY | Changed x/y captures position and current-parent width/height ratios. |
| Size | Changed size or scale captures width/height and both scale axes. |
| Look | Changed alpha, rotation, grayed or touchable captures all four fields. Gray handlers execute before capture. |
| Color | Image, Loader and Text/TextInput color setters capture on changed value. Graph and compound title-color assignments capture even for equal values. MovieClip.color has no updateGear call in this reference. Outline writes capture on the underlying text field. |
| Animation | Explicit changed Playing/Frame captures both live values. Automatic frame reports, rewind and segment configuration do not implicitly capture. |
| Text | Button/Label titles and ComboBox text assignments capture. The inspected GTextField text setter only assigns its display text and does not call updateGear(6). |
| Icon | Changed Loader URL captures; Button/Label/ComboBox icon assignments capture even for equal values. Forwarded child writes retain their own behavior. |
| FontSize | The inspected GTextField fontSize setter and compound titleFontSize setters have no automatic updateGear(9). Controller application/default capture remain supported. |
| Display/Display2 | Connection state stays separate from value storage and explicit visible. |

The text/font/MovieClip-color exceptions are source-specific behavior, not a
generic claim about all FairyGUI backends. Public set_object_font_size forwards
to the actual text field; set_control_outline_color follows native OutlineColor
forwarding. Color snapshots read stored text color rather than the temporary
grayed display foreground.

## Defaults, null pages and binding

GearBase.init takes a default snapshot even when a binary record has no explicit
default. setup_gears now seeds that value before reading instance extensions;
an explicitly exported default replaces it. This fixes native Basics
Demo_Controller.n3/n5 XY records that intentionally omit default data. Button,
Label and ComboBox resource title/icon caches are initialized from their built
children. Empty titles are applied to the actual child instead of leaving an
old visible title behind.
Runtime title and icon setters forward only the assigned property. An icon
change cannot reset independently edited child text, and a title change cannot
reset the child icon or capture its unrelated gear. Construction/selection
refreshes can still deliberately apply both fields.

Saved current-page values are independent of this default. Native JS stores an
unselected page under the `null` dictionary key. Gear.unselected_value represents
that entry; values[None] continues to mean the default fallback. Editing an
unselected page therefore does not overwrite the default used by another page
without a stored value.

Gear.controller_owner caches the owner of the bound controller when the gear is
set up. Reparenting does not bind it to an unrelated controller with the same
index on the new parent. Runtime capture uses the original controller's page;
percent XY still divides by the object's current parent's dimensions. A new
parent's controller pass skips the old binding, and an old parent's controller
pass does not apply to a child no longer in its child list.

These are additive fields on the public Gear data type. Use Gear::new(kind)
when creating data so the new fields receive their initial values. Parsed gears
bind during setup; internal programmatic gears can bind when first applied or
captured. Invalid or removed controller owners do not trigger capture.

## Locks and construction

GObject.updateGear skips an object while under_construct or gear_locked. Locks
are per object, so a locked button's forwarded child-color/icon write or a locked
component's grayed propagation can still capture the descendant's own gear.
Nested core operations preserve an existing caller lock when they return.

Numeric gear tween ticks, completion/retarget writes, immediate value gears and
Transition.applyValue writes lock their own target. Shake offsets and stop-time
restoration use the same policy. The reference's animation-pause and interval
skip setters are outside Transition.applyValue and remain ordinary capture paths.
Automatic movie-frame reports remain outside the explicit Frame setter.

Resource components stay under construction during their controller pass;
parent instance setup marks each referenced child under construction until its
extension data is read. Descendants that are already constructed retain their
own independent capture behavior. Existing initialization also suppresses new
controller gear tweens.

The existing parent/sibling resize relation paths now adjust all saved absolute
XY/Size pages and defaults by the relation delta, then directly recapture the
current value. Percent XY skips that shift, following native GearXY. This does
not replace the project's remaining incomplete relation geometry rules.

## Evidence and verification

Run with the ignored native reference checkout:

```text

```

`gear-updates-reference.txt` executes unchanged type-stripped GObject and control
property methods, native GearBase plus eight value-gear classes, Controller and
Transition.applyValue. Method/class SHA256 hashes are recorded in its header.
The minimal host supplies display properties, trees, normalized initial values,
controller pages and explicit linear tween inputs. Native callbacks perform the
actual locking/property writes. The host does not simulate the native renderer,
relation solver or native GTween scheduler/easing clock.

**40 cases, 279 operations and 1,674 checks** compare live values, defaults and
saved a/b/c/null storage: numeric edits, construction/lock suppression, native
setter exceptions, forwarded title/icon/font state, empty titles/icons, null
pages, percentage XY, reparenting, relation deltas, partial tween edits/retargets
and transition writes. The core also tests parsed Basics defaults, recursive
grayed capture, real parent-resize gear deltas and shake stop-time restoration.
An adapter test verifies exported controller page roundtrips restore Node
geometry, image tint and text color on existing Bevy entities. It also verifies
plain TextFont and LineHeight return to the stored page's font size: the dirty
sync path now updates these components instead of only changing the model/text
string. Rich text continues to rebuild its rendering snapshot when formatting
changes. Bitmap-font resizing is now handled by its own native layout path;
see [bitmap text](bitmap-text.md) for its separate evidence and scope.

Windows smoke uses the unchanged native Demo_Controller package. It edits
geometry, scale, opacity, colors, movie playback and font size, switches away
and back through real gear tweens, and checks the restored properties and same
entities. Font size deliberately returns to its stored page value because its
setter has no capture hook. A separate null-page roundtrip retains its edits
without replacing default fallback. Captures are:

- `target/fairygui-demo-gear-edited.png`
- `target/fairygui-demo-gear-restored.png`
- `target/fairygui-demo-gear-null-page.png`

This phase passed 716 workspace tests, strict all-target Clippy, formatting and
the full Windows smoke of 15 demos twice/20 Basics pages. The three captures were
inspected: edited geometry/opacity/tints survive a page roundtrip, font rendering
returns from the temporary edited size to the stored size, and null-page edits
remain independent. Existing local color-filter captures also passed all 96 GPU
samples, maximum channel error 1/255. The checked-in fixtures record the native
cases exercised by these regressions.

## Remaining scope

The core uses normalized f32 colors and String text/icon values; native null
strings remain normalized to empty strings by the existing package/API model.
These checks cover canonical RGB colors and finite parent dimensions, not every
CSS spelling/alpha form, invalid binding or zero-parent percent combination.
Full native relation/clamping/pivot behavior, controller event notifications,
all reentrant construction/action orderings and renderer gaps remain separate
work. Native framebuffer pixel parity is not established by the trace or smoke.
