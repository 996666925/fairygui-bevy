# Component and button grayed state

The Bevy adapter already renders local image/movie/Loader gray matrices and
text foreground grayscale. The core now also follows FairyGUI-Laya's component
and button handler branches, including instance initialization. Bevy remains
0.20.1, verified as crates.io's latest stable release in this phase.

## Branches and assignment semantics

| Object | Handler |
| --- | --- |
| Button with a `button` controller containing `disabled` | Select `selectedDisabled` when selected and that page exists, otherwise `disabled`. On ungray select `down` when selected, otherwise `up`, independent of recorded hover/press. Stop here. |
| Other component with a `grayed` controller | Select index 1 while grayed and 0 otherwise. Page names are irrelevant. Stop here. |
| Other component | Assign the same grayed value to each direct child. Each child's own handler selects the next branch. |
| Leaf object | Its own grayed flag controls local appearance. |

This applies to ordinary components, buttons without disabled pages, labels,
lists, trees, sliders, progress bars, combo boxes and scrollbars. A controller's
Look gear can select which children actually become gray; uncontrolled siblings
keep their independent appearance. A disabled skin's exported colors remain its
own presentation, without an additional gray filter from its button ancestor.

Native JavaScript does not initialize `_grayed`. Its first assignment of false
therefore enters the setter even though a boolean interpretation of the previous
value is false. The core retains a private assignment bit while public `grayed()`
continues to return a bool. This matters for a source component whose independently
serialized children are gray: the first `set_grayed(root, false)` restores them.

After that first assignment, equal values do nothing. A child can override its
own flag while the parent remains gray; setting the same parent value again does
not restore the child's previous state. Ungraying a parent assigns false to its
children rather than restoring a saved child flag. Newly attached/reparented
children keep their existing flag; parenting does not trigger the handler.
`grayed` does not change `touchable` or introduce a blanket input blockade.

## Construction and ownership

A referenced child resource is built first. The parent's setup_beforeAdd then
reads its instance grayed flag. The parser previously assigned that bool directly
and skipped the handler, leaving nested children and internal controllers stale.
It now handles a newly true instance flag after the initial properties are read,
before setup_afterAdd, instance selection/gears and the parent's controller pass.
Controller gear tween creation is suppressed during this construction operation,
as with native UIPackage construction. Ordinary runtime changes may tween.

Buttons still use the existing initialization/selection machinery after their
instance extension is parsed. In the native selected Button82 example, the
serialized gray flag initially selects disabled, then instance selection drives
selectedDisabled. The final children retain their skin colors. Leaf model IDs,
view entities, material ownership and disposal continue through existing paths.

Component gray changes leaf properties through the object model. An explicit
component color filter now applies to its composited subtree, including Loader
content, through the shared [offscreen compositor](compositing.md). Component
gray itself remains the native controller/child-assignment behavior.

## Native evidence and checks

The checked-in native reference trace records results from unchanged type-stripped native
GObject grayed accessors, GObject/GComponent/GButton handlers, GButton.setState,
the Controller class and GearLook.apply under a minimal host. Source method/class
SHA256 hashes are recorded in `grayed-reference.txt`. The host supplies object
trees, controller data, immediate Look gears and a matrix-selection recorder.
It does not emulate rendering, input, timers, controller events or per-page
runtime gear-value capture. Rust tests consume the checked-in reference snapshots.

The core compares **27 cases, 76 operations and 273 object states**: nested
propagation/restoration, first false, equal assignments and child overrides,
controller stops and Look gears, selected/unselected buttons with absent or
partial disabled pages, component-derived widgets, late attachment and reparenting.
Separate tests cover construction-time tween suppression and touchable independence.

`Grayed.fui` is deterministically derived from the official Basics export:

```text
python tools/generate_grayed_package.py
```

Only package identity/URL IDs, dependency metadata and 22 referenced component
instance grayed flags in Demo_Component/Demo_Button change. Controllers, gears,
selection records and atlas filenames remain native. Construction tests inspect
actual nested Image/Text leaves, Button's fallback, Button52's grayed controller
and Button82's selectedDisabled/disabled pages. Adapter tests verify resulting
gray material rows, #AAAAAA foreground, unchanged controlled skin colors and
retained leaf entities after ungray. Test atlas handles stay owned through sync.

Windows demo smoke opens both derived component/button pages, captures serialized
gray state, applies the first false assignment on their roots and captures the
restored views after several rendered frames. It checks controller/leaf state,
entity reuse and disposal. Captures are named
`target/fairygui-demo-grayed-{Component,Button}{,-restored}.png`.

This phase passed 704 workspace tests, strict all-target Clippy, formatting and
the full Windows smoke of 15 demos twice/20 Basics pages. All four captures were
inspected: panel/image/text gray restores to original color, the controller-based
button retains its own colored skin, and disabled/selectedDisabled skins switch
back to enabled presentation. Existing local color-filter captures also passed
all 96 GPU samples, with maximum channel error 1/255. These are Bevy rendering
checks and native model traces, not a native framebuffer pixel comparison.
The checked-in fixture records the native state transitions used by the tests.

## Remaining scope

The grayed setter now calls the runtime Look-gear capture path, including local
locking and construction suppression; see [gear state](gear-state.md). The
grayed-only trace host still omits storage recapture, while the newer gear trace
executes native updateGear/updateState. Controller events,
malformed controllers with fewer than two grayed pages and every construction
callback ordering combination are not established by this trace. Renderer limits
for text pixels, mask bounds and PostProcess2D remain documented in
[compositing.md](compositing.md) and [color-filters.md](color-filters.md).
