# Runtime architecture

Applications own interfaces through `FairyGuiRoot` components, find controls with
`GuiQuery`, attach entity observers and send `GuiObjectCommand` messages from
ordinary Bevy systems. The core object tree executes FairyGUI package semantics
internally; the plugin projects its state into Bevy UI and read-only ECS components.

## Responsibilities

| Layer | Responsibility |
| --- | --- |
| `fairygui-core` | Package parsing, object ownership, widget state, scrolling and animation; no Bevy dependency |
| `asset` | Bevy asset loading and package resource paths |
| `ecs` | Root-to-view references and read-only identity, hierarchy, geometry, text, display and scroll queries |
| `popup` | Entity-addressed popup/menu commands, pending asset requests and popup stack snapshots |
| `list` | ECS virtual-list configuration, restricted synchronous row renderers and layout diagnostics |
| `pull` | Entity-addressed refresh locks applied between Respond and Advance |
| `runtime` | Application facade, object/entity lifecycle, event bubbling and view synchronization |
| `runtime::windows` | Asset-backed ECS window ownership, entity commands and native window snapshots |
| `runtime::global_wait` | Resource-configured stage-wide wait ownership, native stack ordering and input gating |
| `backend`, `text`, `movie` | Render representation, text layout and movie playback |
| `core::graph`, `graph`, `polygon.wesl` | Native polygon points/stroke geometry, retained Bevy UI material surfaces, coverage and contour hit validation |
| `core::fill`, `image_fill`, `image_fill.wesl` | Native fill polygons, image/movie UI material masks, atlas/slice mapping and retained material ownership |
| `runtime::appearance` | Current model tints/opacity for images, graph fills, text spans and bitmap glyphs |
| `core::text::markup`, `core::text::ubb` | Native UBB conversion, HTML/XML element/style parsing and renderer span projection |
| `runtime::movie_sources` | Loader image/movie source changes and frame-to-atlas bindings |
| `runtime::loader_status` | Loader state observation, explicit retries and native FIFO error-sign ownership |
| `runtime::stage` | Stage visibility, logical parenting and removed-view cleanup |
| `runtime::rich_text` | Bevy rich-text span rebuilding, global loaded-package images, link hit geometry and linked sibling-image ownership |
| `core::bitmap_text`, `runtime::bitmap_rich_text` | Native bitmap command layout, UBB style/image/link mapping, logical metrics and owned bitmap parts |
| `input` | Bevy pointer sampling, hit validation, capture and event routing |
| `input::gestures` | Exclusive slider, scrollbar, window or content drag ownership |
| `input::actions` | Default click behavior for windows, dropdowns, lists and trees |
| Application | Business actions and navigation; the demo uses this same public API |

The Bevy integration exposes FairyGUI events as Bevy entity events. Applications
attach observers to the entities they own; there is no numeric click-listener
registry or compatibility layer for an older API.

`GuiPopupCommand` accepts root owners or view entities for existing popups and
menu template sources. Commands run after root reconciliation and object commands
in Sync. Requests for roots awaiting assets retain the source component identity;
replacement, removal and disposal cancel them. Hide cancels that target's pending
shows, HideAll clears all pending shows, and DisposeMenu clears pending menu shows.
Within the popup message stream, operations keep their order. The runtime retains
one menu configuration, with checked state and edits preserved on reopen.

`GuiPopupState` exposes popup views from bottom to top and the open menu view,
refreshed before Publish and at the end of Sync. Windows are tracked separately.
Positions use stage logical pixels. Commands refresh the stage viewport from the
window selected by `GuiInputSettings`; renderer-free apps preserve an existing
stage or default to 1280x720. Template resolution starts with the source object's
declared package, with registered cross-package URLs following normal rules.

`GuiVirtualList` is attached to the list view entity. The plugin reconciles changed
configurations before virtual layout and view spawning. Its `Send + Sync` renderer
receives a logical row index and temporary `GuiListItem`, which exposes row/title
updates, child paths, controllers, icons, pivot and synchronous dimensions without
runtime or object IDs. A new item count is installed before the new renderer runs,
so shrinking data and replacing its renderer cannot access old indices.

`GuiVirtualListState` projects current virtual geometry diagnostics and the latest
configuration error after Sync. Removing the configuration empties the logical
list and drops its callback; native FIFO row pools remain owned by the list and
are reclaimed on disposal. Reinsertion can reuse them. Loop mode is fixed at
initialization, and a conflicting configuration reports `LoopModeMismatch` while
preserving the existing renderer. Business systems update the component or mark
it changed when captured/shared data changes; ordinary frame refreshes reuse rows
and do not re-run the renderer unless native layout requests it.

`GuiListState` is projected for ordinary lists, virtual lists and trees. It exposes
the logical count, materialized row entities in native child order, and the native
first item in view. Pane overlays and pooled rows are excluded. `GuiListRow` gives
each active row its list entity, logical index and optional virtual record index.
Recycling changes the binding on the same entity; pooling removes membership.
Unchanged list and row snapshots preserve their change ticks.

`GuiSystems::PostLayout` is nested inside Sync, between native list refresh/view
spawning and final property projection. Fresh hierarchy, geometry, text, display,
scroll and list snapshots are available there. Application decorators can query
the actual retained rows and send scale/text commands for the current frame; the
carousel demo uses this path without borrowing the runtime. Both object-command
passes share one cursor and pending-root queue, so early commands are not replayed
and playback generations/one-shot effects remain unique. Changes that influence
the virtual range or row creation belong in Respond, before native layout.
PostLayout precedes Bevy PostUpdate layout and text measurement.

`GuiListContent` configures every ordinary List row with the same restricted
`GuiListItem` renderer used for virtual rows. It preserves leading row identities,
creates growing rows from package-aware default templates, and disposes trailing
rows on shrink. Templates are constructed and prepared before attachment; an
unresolvable/failed growth leaves existing rows untouched. Registered pane
overlays never participate in data counts or disposal. Removing the configuration
releases its renderer and keeps the current content; count zero clears data rows.
`GuiListContentState` reports configuration errors. Trees and virtual lists reject
ordinary configuration; an attached `GuiVirtualList` takes precedence. Unchanged
components do not rerun renderers. Mark changed or replace to retry/update data.

`GuiControllers` stores controller page tables, selected/previous indices and
selected page identity, refreshed before Publish, PostLayout, and at Sync end.
Controls without controllers omit the component. Indirect controller actions
therefore appear through ECS before application input handlers run.

`GuiPullRelease.entity` identifies the original scroll container view. The scroll
snapshot includes header/footer view entities, current/exported sizes, refresh
axis and lock extents. `GuiPullCommand` locks a materialized header/footer using
logical pixels, with zero releasing it. Commands execute in stream order after
Respond and before Advance so the first release tick uses the response boundary.
They discard missing targets and invalid sizes; no asset-waiting requests are
retained. The pull demo uses ECS for its controller pages, timed jobs, ordinary
row growth and refresh locks.

`FairyGuiWindow` is an independent owner component, separate from `FairyGuiRoot`.
Its asset/item identify the native window object; visible/modal/draggable are live
configuration. Construction waits for assets, recognizes native frame parts, and
centers once by default. It also supplies an asset fallback for the synthetic
stage when a window is the first GUI in an app. Native row preparation still runs
at view creation so application initialization can replace authored list content.

`GuiWindowCommand` targets the owner or root view with Show/Hide/BringToFront/Center.
Pending asset requests retain template identity, and replacement, component removal
or despawn cancels them. Object commands also resolve window owners and preserve
pending requests. Window reconciliation runs after root reconciliation; its command
stream follows object commands and precedes popup commands. Center refreshes the
viewport from GuiInputSettings. There is no business-visible window slot index.

`GuiWindowState` is projected on owner and view before Publish and at Sync end,
along with GuiRootView for hierarchy/property lookup. Native close-button input
hides ECS windows, retaining views and reflecting visible=false on the component.
Removing the component or despawning the owner disposes its model and views.
Modality updates the native root entry without reordering or detach/attach events.
Drag capture retains the model object identity, so deleting another window cannot
redirect an in-progress gesture through a compacted window array. Hiding or
disabling drag cancels the gesture. Basics/Window uses entity observers and
retained child owner entities for its two exported windows.

Window modal waiting uses `FairyGuiWindow.modal_wait` as a lazy local/cross-package
template. ShowModalWait records a request token; CloseModalWait with a nonzero
token only closes that request, while zero is unrestricted. GuiWindowState exposes
waiting status, token, pane entity and the unresolved/failed template reference.
Failed template construction retains the logical request without a blocking view;
another Show retries after dependencies are registered. With no template only the
request is recorded. Native source: `Window.ts` lays out the wait pane within the
frame's contentArea, appends it above content, and closes waiting on UNDISPLAY.

Explicit frame/dragArea takes precedence; assigning an empty GGraph drag area
creates the native invisible rectangle hit surface. Wait layout converts the
frame origin to window coordinates and adds the contentArea offset, retaining
the native unscaled content dimensions. The title and close button stay outside
the wait pane. Close detaches and hides the retained view, Hide also closes the
request, and window disposal includes detached wait panes and their descendants.
Changing the template clears and disposes the previous pane. ModalWaiting's
window buttons and delayed completion use only ECS observers, components and
commands; `--smoke-window-wait` verifies actual input blocking, title dragging,
timed completion, pane reuse, close-during-wait and disposal.

Popup menu template sources and pull-lock targets also resolve window owners.
Window-backed pending menu requests retain asset/item identity and cancel on
replacement/removal/despawn just like root requests. Plain popup Show/Hide rejects
native window objects to preserve their distinct visibility and modality state.
Plain GTextField nodes are excluded from pointer hit candidates, matching native
mouseEnabled=false even if their logical touchable flag is true. Ancestor controls
remain candidates; rich text links and input fields keep separate hit/edit paths.
This prevents title text above a sibling dragArea from stealing the window drag.

`GuiGlobalWaitSettings` selects one asset-backed template for the stage-wide native
wait pane. Show records the optional native text and retries when assets or
registered dependencies arrive; failure is projected through GuiGlobalWaitState,
and never blocks input before a real pane is attached. Hide cancels a pending
request and detaches the retained tree. Dispose reclaims it even when hidden.
Template replacement/clearing disposes the old tree; an outstanding Show attempts
the new configuration. The state resource separates requested from actually
waiting, retains the hidden view identity, and preserves unchanged change ticks.

The global wait reconciler runs after window/popup commands and before view
spawning. It sizes to the selected input window, appends the pane above native
windows/popups, and projects GlobalZIndex(i32::MAX) for independent UI roots.
The common modal input gate then accepts only descendants of an attached visible
wait pane. Pointer/touch capture, wheel routing, drop receivers and text focus
use this same policy. Window modality and GuiPopupState remain independent.
The native references are GRoot.showModalWait (size, relation and native text),
closeModalWait (retained removal), and adjustModalLayer (keep the wait pane last).
Renderer-free apps preserve stage dimensions, with a 1280x720 initial fallback.

ModalWaiting's page owns a three-second demo timer through an ECS resource; the
page's removal cancels the timer and hides the global pane. Its real-window smoke
clicks the covered Open Window button, checks the pane after resizing, waits for
restored interaction, then continues the existing window-local wait scenario.

## Frame contract

Bevy 0.20 picking updates `Hovered` during `PreUpdate`. FairyGUI then runs these ordered
`Update` sets:

1. `GuiSystems::Input`: choose a target, update capture, apply built-in behavior.
2. `GuiSystems::Publish`: refresh text/display/scroll snapshots, trigger routed `GuiEvent` entity events and publish rich-text `GuiLink` messages.
3. `GuiSystems::Respond`: application systems consume messages and send UI commands.
   Pull lock commands are applied immediately after this set, before Advance.
4. `GuiSystems::Advance`: advance builds, scrolling, transitions and animations.
5. `GuiSystems::Sync`: reconcile ECS root ownership, apply commands, spawn pending views, refresh logical snapshots, run the nested `PostLayout` application set, apply its object commands, synchronize final state components and play sounds.

Application changes in `Respond` reach view synchronization in the same frame.
Bevy layout still runs later; hit geometry and text measurements reflect the last
completed layout. Chain application systems if they depend on each other.

`GuiRootView`, `GuiObjectInfo` and `GuiTransform` are updated at the end of Sync. Systems in
Respond read the previous snapshot; systems after Sync can find newly created
controls and bind their observers. Logical children follow FairyGUI order and
exclude generated render nodes. Detached, hidden and pooled views remain
queryable until disposal. Unchanged hierarchy snapshots preserve ECS change ticks.

`GuiTransform` reports logical model geometry: position relative to the FairyGUI
parent, unscaled size, scale, fractional pivot, anchor mode, rotation and skew in
degrees. It excludes scroll offsets, pivot compensation and ancestor transforms;
use Bevy layout components when actual rendered geometry is needed. Snapshots
include indirect updates from relations, groups, controllers and animation.
`GuiQuery::transform` resolves either a root owner or a control view, so layout
systems can read geometry and send commands without borrowing `GuiRuntime`.
`GuiFont` is captured before ECS root construction and view spawning, including
the first update and resource changes before later roots are created.

`GuiText` retains the native text property before translation, markup or password
presentation. Button text is its base title; labels and combo boxes forward to
their title children. Inputs expose committed values, excluding prompts and IME
preedit. Unsupported controls have no text component. `GuiDisplay` separates
explicit visibility, controller/group drawing visibility and stage attachment;
it also reports local/effective alpha, grayed and touchable. Local drawing
visibility excludes ancestor visibility, clipping and occlusion.

Text and display refresh before input events are published as well as at the
end of Sync. Observers can therefore copy the text committed by this frame's
Input into another entity using `GuiObjectCommand`, without runtime borrowing
or an extra frame of delay. Unchanged snapshots preserve change ticks.
`GuiLink.entity` identifies the original control entity; its core payload is
retained alongside the entity reference.

`GuiScrollState` keeps logical target and current display offsets separate during
programmatic animation, with view/content sizes, maximum offsets, logical/display
percentages, current pages and animation activity. It refreshes before Publish
events and after Sync, and is absent on controls without a scroll pane.
`ScrollTo`, `SetScrollPercentage` and `ScrollToView` invoke the native model's
programmatic scroll policies. Percentages clamp to 0..=1 and resolve against the
current range when commands execute; callers do not need a potentially stale
maximum from the preceding snapshot. All coordinates are logical pixels.

```rust
use bevy::prelude::*;
use fairygui_bevy::prelude::*;

fn react(event: On<GuiEvent>, mut commands: Commands) {
    if event.event().event.kind == fairygui_bevy::core::object::EventKind::Click {
        commands.write_message(GuiObjectCommand::new(
            event.entity,
            GuiObjectAction::SetText("Clicked".into()),
        ));
    }
}

fn attach_observer(commands: &mut Commands, gui: &GuiQuery, root: Entity) {
    if let Some(entity) = gui.child_by_path(root, "n1") {
        commands.entity(entity).observe(react);
    }
}
```

Attach observers to control entities returned by `GuiQuery`. Click events
trigger once on the original target and again on each logical ancestor while
bubbling; each observer receives its current entity in `GuiEvent::entity` and the
complete FairyGUI payload in `GuiEvent::event`. `dispatch_event` accepts a full
event and preserves its pointer/data payload while bubbling.

Movie players read Playing/Frame commands before advancing and report their live
frame back to the core store. Empty texture frames retain their timing and index;
view visibility hides them without pausing their timeline. Loader source changes
keep the same entity. Render appearance is recomputed from model colors and
effective alpha, preserving explicit colors on rich spans.

Movies and loaders keep a full-size object root for layout, pivot and input. A
non-pickable child draws the actual image. Loader fill/alignment uses the original
content canvas first, then movie-frame trimming applies its scaled offset and
crop dimensions within that canvas. Blank frames hide only the child. Auto-size
reads original content dimensions before dirty object synchronization. Source
replacement removes the old child while preserving the root entity.

Package-backed objects inherit their resource dimensions before optional instance
dimensions are read. This also supplies source dimensions when the exported child
omits a size override. Group movement traverses sibling members, including nested
groups. Group alpha assigns member alpha as in the official runtime; component
ancestor alpha still accumulates normally.

`GObject::visible()` is explicit application visibility. `display_connected()`
adds controller display gears and display locks; `display_visible()` combines the
two. `ObjectStore::display_visible()` also follows FairyGUI groups, which are
siblings rather than entity ancestors. Input and rendering share this policy.

Button clicks resolve to the nearest button ancestor. The core applies Common,
Check and Radio selection rules before list/tree/popup actions; input publishes
StateChanged only for a click that changes selection, retaining pointer position.
Programmatic selection changes update visuals and related controllers without
that event. Controller page changes synchronize directly related buttons in the
opposite direction, including selection titles/icons and optional group depth.

Transition playback binds target handles when the component is constructed,
preserving identity across child reordering. The driver splits long frames at
item starts/ends and loop boundaries, retaining leftover time. Resolved tween
endpoints capture omitted XY/size coordinates when an item actually starts;
percentage positions use owner dimensions and owner-targeted XY uses the origin
captured at the start of each pass. Exported paths offset the captured start.
Completed items stop writing properties while other tasks finish.

Nested playback uses an operation-local transition graph, removing only the
currently driven transition. This permits same-owner calls while preserving all
other definitions. Shared frame clocks split at events and advance each animation
only through the remaining portion of that frame. Parent completion waits for
child completion notifications, including repeated or multi-level playback;
parent loop reentry occurs at that actual completion time. Zero-duration loops
yield once per tick and active-call cycles are rejected.

Nested items with times=0 follow the reference's stop-marker rules: a paired item
sets the preceding call's per-pass end breakpoint, while an unpaired marker plays
only the zero-length start interval. Breakpoints propagate to grandchildren and
retain interpolated values instead of forcing endpoints or firing incomplete
end hooks. Parent stops propagate completion/cancellation to resolved children.
Replay interrupts notify the previous parent. A separate child stop defaults to
processCallback=false, matching native behavior; its parent continues waiting.
Use `stop_transition_with_options(owner, name, complete, true)` to send that
completion notification explicitly. Scoped auto-play starts only the newly
spawned root's subtree so existing roots retain their progress.

Transition pause preserves timeline progress and display locks, recursively pauses
resolved child transitions, and snapshots/restores Playing on animation targets.
Changing speed preserves progress and propagates the same rate to active children
and animation targets; rates do not multiply along the nesting chain. Frame
clocks remain wall time while each transition converts its own intervals to local
time. The startup delay follows the reference's unscaled delayed call; pause
freezes that delay. Speeds must be finite and nonnegative, with zero freezing the
internal timeline. Movie players apply their own model rate without resetting
fractional frame time.

Reverse playback schedules items from nominal total_duration, walks items in
reverse order, and swaps tween endpoints while retaining repeat/yoyo and ease.
Discrete actions apply the same exported values at their reversed times. Exported
paths retain their shape relative to the swapped start, matching the reference;
this is not a geometric reversal of the curve. Child transitions inherit reverse
mode and speed. play_transition_reverse starts one reverse pass, as the reference
playReverse does.

`TransitionPlayOptions` adds start/end timeline positions alongside count, delay
and direction. Initialization samples the start directly in source order rather
than replaying historical events. Historical property assignments apply, old
sounds and start hooks are skipped, and completed historical tweens can still
emit their end hooks. Breakpoints clip interpolation without emitting an
unfinished tween's end hook. Each repeat recaptures the owner's origin and starts
at the configured position. Nested ranges subtract the source item's original
time even during reverse playback, and paired stop markers further bound the end.

Forward nonzero starts batch historical Animation items by target, accumulating
only playing segments and discarding earlier accumulation at explicit frame
resets. Reverse playback does not perform this catch-up. This path intentionally
preserves the movie's timeScale. The model queues ordered `AnimationCommand`
operations for the adapter, retaining the initial frame when the first operation
is an advance so delayed asset creation can replay from the correct state.
Unresolved sources retain commands; resolved static/empty loaders discard them,
and object removal drops their queues. Bevy consumes commands before normal frame
updates, even for hidden/stopped movies. `MovieClipPlayer::advance_time` implements
unscaled DeltaTime, clears fractional time and uses the reference's first-return
cycle calculation with millisecond frame durations. This differs from using a
steady-state loop duration when the first loop has no repeatDelay. An additional
cycle guard handles malformed zero-duration data and unreachable initial states.

Ordinary movie updates clamp input to 100ms before multiplying speed, take at
most one step and cap retained elapsed time to the base interval. They honor the
reference's active/next-loop/ending/ended state machine; a loop restart clears
elapsed time and overrides the underlying swing step. DeltaTime bypasses these
range states, matching the reference's separate advance method.
`AnimationPlaySettings` reaches the player through ordered commands. Finite ranges
hold their end until the next frame-timing threshold, then position at end_at and publish one
`GuiAnimationComplete` in Advance. Completion leaves Playing unchanged; frame,
Playing and rewind do not rearm an ended range. New settings rearm it without
changing speed/direction/repetition history. Endpoints are clamped to valid clip
indices. `rewind_animation` resets frame/timing/direction/repetition, while the
adapter's `sync_status` copies only timing state from another player.

Shake tasks are created even beyond the selected end. Their nonnegative breakpoint
is end_time minus the item's original time, including during reverse scheduling;
a negative result disables truncation. The scheduler therefore uses per-task ends,
and some shakes continue beyond the selected end. Ending a shake restores its
offset immediately and emits no start hook. Other clipped tweens retire at their
breakpoint, so they do not overwrite later property edits while a shake continues.

Tween repeats/yoyo and shake duration determine actual playback completion;
`total_duration` retains the exported nominal timeline length. Shake replaces its
previous offset, so external movement survives completion/cancel. Transition
option 1 acquires display locks for non-owner targets and releases them on stop
or completion. `ObjectStore::stop_transition` completes pending property work;
`stop_transition_with_completion(..., false)` retains current property values.

`endLabel` names an ending hook. It never redirects playback. The core queues
`TransitionHook` records; Bevy publishes `GuiTransitionHook { entity, hook }` and
`GuiTransitionCallback { entity, callback }` at the end of Sync, once pending
views and state snapshots are available. Systems after Sync can read them in the
same frame; Respond sees them next frame. The demo uses these notifications for
PowerUp's counter without runtime borrowing.

`GuiTransitions` projects each exported transition in package order, including
playing/paused/reversed, time, nominal duration, speed and playback generation.
`GuiQuery::transition` accepts root owners and view entities. Snapshots refresh
before Publish and at the end of Sync and preserve change ticks when unchanged.
`PlayTransitionWithOptions` exposes the native interval/delay/repeat API through
entity commands.

Natural completion is queued at the actual core completion branch after repeats
and nested tasks finish, and published as `GuiTransitionComplete` after Sync.
This captures instantaneous playback and multiple playbacks within one frame.
Each record carries the owner's entity, transition name and playback generation,
so consumers can reject completion from an earlier replay. Stop, replacement,
stage detachment and disposal do not synthesize natural completion. Disposing
an owner reclaims its queued completion records. The demo restores its button
group and despawns its sample owner by consuming these messages; explicit stops
are handled separately through the ECS playback state.

Transition edits use `TransitionEditValue` instead of dynamically coerced JS
arguments. Value edits affect every matching start/end label (start wins within
one item); duration and target edits match start labels only. Label-time queries
return the first source-order match and use current definition duration without
changing exported total_duration. Invalid numeric/type/target edits return an
error; a value edit validates all matches before writing any of them.

Each pass captures task scheduling, tween values/durations and Shake parameters.
Initial seek/capture follows source order, so immediate Size actions can move
later absolute XY definitions through relations before their values are captured.
Pending absolute tweens retain their copies; missing axes and child percentage
coordinates resolve from live definitions/object state on start. Existing active
tweens keep interpolating their captured values when retargeted. Discrete actions
read current values when they fire. Nested bindings/stop markers are resolved for
the play operation and retained across repeats; edited names/targets take effect
on replay while play count remains live at the nested cue. First-triggered sound
addresses stay cached across edits/replays, matching the reference's audioClip
behavior while volume remains live.

User hook bindings route one listener id through `TransitionCallback` and Bevy's
`GuiTransitionCallback`. They use first-match start/end precedence, can be replaced
or cleared during playback, and remain separate from existing automatic label
notifications. Hooks do not store or invoke application closures inside the
transition driver. Relation-driven movement updates absolute XY definitions in
the parent's transitions while preserving percent coordinates; driver-local
pending relation deltas allow a currently sampled transition to receive its own
layout changes without aliasing the store. Transition count zero is one pass;
only negative counts repeat infinitely, unlike MovieClip's zero-count loop rule.

The core tracks external stage roots separately from its convenience `root()`
identity. `attach_to_stage`/`detach_from_stage` connect independent trees;
parenting into a staged tree connects descendants. Stage membership follows
Display/Display2, display locks and group connection, not explicit visible or
alpha. GGroup has no stage sprite. Gear application batches each object's gears
before stage notifications, avoiding intermediate Display/Display2 disconnects
and allowing numeric gear display locks to preserve connection.

Stage entry starts auto transitions only if stopped. Stage removal stops without
processing completion callbacks: option 2 disables this stop and overrides option
4, which otherwise completes pending values before stopping. Dynamic auto-play
changes follow the reference: equal flags ignore new count/delay, enabling on
stage replays, enabling offstage only configures, disabling onstage lets the
current play continue, disabling offstage stops without completion and notifies
its waiter. Stage events generated while a transition is on the driver stack are
queued and drained alongside nested completion notifications. Explicit disposal
cancels transitions regardless of the removal-stop option.
Transition display locks are acquired after initial item playback, so a child
connected by a lock sees any nested play already running. Locks remain held
through repeat passes and are released only at final completion or stop.

Bevy mounts built top-level roots after view creation. `set_root_attached`
preserves entities across unmount/remount; new roots can be configured before the
asset/view is ready. Layer roots created by ensure_root are connected explicitly.
`sync_stage_hierarchy` mirrors logical ChildOf links, hides disconnected views and
cleans stale mappings. It updates surviving relationships before despawning old
parents, protecting detached pooled children. Pointer eligibility requires stage
membership. Ordinary movie timers use stage membership too: explicit hiding does
not pause movies, offstage ordinary updates pause, and queued DeltaTime still runs.

Window hide uses Root::detach and retains the full subtree; re-show mounts the
same objects/entities. Close disposes and removes all descendant mappings. Layer
reordering uses child indices rather than detach/re-add, so bring-to-front does
not stop or restart transitions. Repeated show/hide operations are idempotent.
Explicit root/window/subtree removal repairs surviving view parents before queued
recursive despawn. Menus and dropdowns retain their complete model/view subtrees on close. Their
item data refills through the object pool only when changed; owner disposal reclaims
cached dropdowns.

Inline images have two distinct relationships: ChildOf places each image beside
its text node for UI layout, while InlineImageOwner links it to that text entity.
The inverse InlineImageParts relationship uses Bevy 0.20 linked_spawn, so despawning
text also reclaims its sibling images. Stage sync moves images to the owner's
current logical parent (or no parent for standalone roots), copies sibling order
and explicitly mirrors text visibility. Explicit removal protects surviving owned
images alongside their text roots before recursive parent despawn.

Image anchors are real in-flow InlineBoxes in the text hierarchy. Their original
canvas sizes, plus Laya's horizontal side margins, participate in text measurement
and line breaking without dummy glyphs. Image positions use the current inline-box
rectangle and physical-to-logical scale, including the text content inset and
sprite trim offset. Missing owner/layout/box data keeps the image hidden; old box
entity IDs cannot expose newly rebuilt images. Sibling center compensation
makes the image quad follow the text's rotation, scale and pivot transform rather
than rotating about an unrelated center. Tint remains texture-white with effective
owner alpha/filter, independent of text foreground color.

UBB text retains a RichTextBlock snapshot even when initially plain. Changes to
translated content, field style or GUI font rebuild only tracked spans/inline boxes
and owned images; ordinary GObject children remain. Rebuilds preserve the original
package id and asset directory after reparenting, including duplicate package IDs.
View creation also reuses mapped entities adopted into pending roots, avoiding a
second entity and an accidental change of asset ownership. These checks establish
lifecycle/state/transform behavior and real image advances. Line metrics/baselines
still come from Parley and are not native HTML pixel parity.

Static external inline images resolve through AssetServer using the source path
as written, independently of the package directory. Strong snapshot handles keep
requests alive and reuse failed handles; natural dimensions participate in snapshot
comparison. Arrival/removal/size changes reflow text-owned parts. Each explicit
HTML axis is retained while automatic axes use the loaded texture dimensions;
pending/failed views reserve explicit space and remain hidden. Bitmap rich text
shares resolution and sizing but owns child quads instead of paragraph siblings.
See docs/external-inline-images.md for reference evidence and loading limits.

Plain bitmap layout exports underline stroke rectangles separately from glyphs.
BitmapDecorations tracks their view ownership without changing BitmapGlyphs'
sampled-image contract. Text/size changes replace both tracked groups; renderer
switches and recursive despawn reclaim them while preserving application children.
The shared stroke-bound helper accounts for native line height, thickness and
strike center truncation. Decoration colors use TextTintSource with inherited
foreground even for untinted fonts. ObjectStore::set_object_underline forwards
compound titles and marks TEXT only on change; ordinary paragraph synchronization
updates Underline as well. See docs/bitmap-text.md for native fixture scope.

TextFontBinding also tracks a selected ordinary FontSource and optional strong
external Font request. Native resource-name detection selects AssetServer loading;
ordinary names become CSS family candidates followed by GuiFont/default fallback.
System font discovery is enabled. Font storage availability changes source
selection independently of model dirty flags; failed handles are reused. Dirty
sync preserves the selected source. RichTextBlock snapshots it and propagates it
to all rebuilt spans. Plain inputs change TextFont in place, retaining EditableText.
Package bitmap selection still takes precedence. See docs/raster-fonts.md.

In PostUpdate, image positioning runs after Bevy's text system and before UI
clipping. It mirrors the final physical quad from the owner's current text-box
geometry and global transform, so a newly measured/reflowed image renders in the
same frame. Computed size is cleared if the anchor is missing. Atlas trim offsets
are applied inside the original reserved canvas rather than stretching the cropped
atlas rectangle across that canvas.

Root creation registers the source package for URL resolution, including async
builds. Sound playback resolves the asset directory of the referenced package
instead of borrowing the first root's directory. WAV and MP3 decoding are enabled
explicitly: Bevy's default audio feature only enables Vorbis. Bitmap fonts accept
package-local URLs as well as names. Rebuilding glyphs uses the owning root's
asset, and a font without tinting preserves its texture colors while still
following alpha/filters.

## Pointer behavior

One router selects the highest Bevy stack entry among eligible GUI hits. Visibility,
touchability (including ancestors), modality, child hit areas and pixel masks use
the same policy for clicks, wheel input and drag starts. Events bubble once from
that target. Button visuals and composite widget actions resolve through ancestors.
Pointer movement emits `MouseMove` on the current hit. Movement while either mouse
button is held emits `MouseDrag` on its captured target, followed by `MouseDragEnd`
after the matching button-up event when the capture ends.
Bevy touch pointers use their own `HoverMap` hit results and independent captures.
`TouchBegin`, `TouchMove` and `TouchEnd` retain the finger ID on `GuiEvent`; touch
press/release also route primary mouse events so built-in widgets, list selection,
and gestures work on touch screens. Policy invalidation and OS cancellation release
that finger without synthesizing a click.
`UiEvent::touch_id` stays separate from `EventData`, preserving link URLs and list
item payloads. `captured_touch(id)` and `hovered_touch(id)` expose individual
fingers, while `wants_pointer_input()` includes all captures. `captured()` prefers
the left mouse button, then the right button, then the lowest active touch ID.
Mouse picking uses only the mouse's hover map; another finger cannot redirect it.
Each slider, window or scroll pane has one gesture owner at a time, and distinct
widgets can be manipulated concurrently. Releasing one finger does not clear a
button still held by another pointer. To bind an application drag-and-drop to a
finger, call `start_touch_drag` with its event ID; `start_drag` uses the mouse.
When a drag ends over a live stage target, `GuiRuntime::finish_drag` dispatches
`EventKind::Drop` to that target and its logical ancestors. The event carries the
source and optional application data without consuming the returned `DragResult`;
touch-owned drops also retain the finger ID. Ending outside the GUI or canceling
the owning pointer produces no Drop event.

A left-button edge captures the target and may begin a built-in gesture. A
right-button edge captures independently and emits right-button events without
running left-button widget actions. Holding a button over a different object
cannot start a second capture for that button. Gesture priority is
slider/scrollbar, window title area, then scroll content. Scrollbar arrows and
track clicks cannot acquire grip dragging. Capture survives leaving the object,
but ends on release, loss of focus, loss of cursor, removal, hiding or a
modal/disabled-state change.

The default drag threshold is six logical window pixels. Crossing it permanently
suppresses the click for that press, even if the pointer returns to its starting
position. Content scrolling only starts after that threshold. Sliders respond
immediately and continue using their relative cursor coordinates while captured.
Wheel line units use the core scroll step; pixel units retain their pixel distance.
Window and scroll drag deltas account for ancestor axis scaling.

Use `GuiInputSettings` to change the threshold or select the input window (the
primary window by default). `GuiInputState::hovered()`, `captured()`, and
`wants_pointer_input()` let gameplay systems arbitrate pointer ownership after
`GuiSystems::Input` without inspecting UI entities.

The adapter routes one mouse pointer plus concurrent touch pointers within the
selected UI window. Simultaneous input across multiple windows is not implemented.
Touch hit candidates come from Bevy's latest completed picking pass, with per-event
positions transformed into the node's local coordinates for geometry and gestures.
Bevy remains responsible for rectangular hit geometry and clipping;
pixel rejection does not bypass unrelated Bevy UI nodes with blocking `Pickable`
policy. GUI nodes allow lower picking candidates so FairyGUI can reject pixel masks
and disabled objects before choosing a target. Decorative glyphs and inline images
are ignored by picking; rich text spans preserve hovering of their owning widget.

## Validation

`cargo test --workspace --offline` covers the existing package/widget tests and
headless input routing regressions. The runtime suite builds all bundled demo
packages. `cargo clippy --workspace --all-targets --offline -- -D warnings` checks
the library, tests and demo. These checks do not substitute for a visual demo run.

Rich-text links use the selected pointer hit and the existing capture lifecycle.
The owner's normalized cursor position includes inverse UI transforms; its
physical content inset is removed before testing individual run/inline-box rectangles.
Links spanning styles/lines share a parser tag identity, while adjacent tags with
the same href stay distinct. Capture records the rich-text generation as well as
the link identity, so rebuilding cannot activate an old link. Link activation
queues `EventKind::Link` with `EventData::Text(href)` from the original text object
and bubbles through its model parents. `GuiLink` publishes that activation once in
Publish, independently of draining the core queue. `event:` is preserved; the
application owns URL policy and navigation.

Inline-image URLs resolve against all loaded FuiAssets. Own-package/bare names
use the text's original resource directory; foreign packages resolve using their
own atlas directories. Dependencies are snapshotted and checked during rich-text
sync: loading or unloading a referenced package rebuilds the affected paragraph,
while unresolved, unchanged dependencies retain their views.

## Button effects and linked popups

Button pointer flags are distinct from visual state. Hover changes only apply
when the button has an over page; holding the pointer preserves the current state
through rollover/out. Common buttons change state on press/release. Check/Radio
buttons retain their selection-driven state until a selection/hover change.
Selection and grayed-page refresh also feed the same visual-state application.

Down effects mutate the core model, as GButton does. Effect 1 writes the supported
Color property of non-text direct children and restores white; plain components
have no Color property, while labels/buttons forward it to their title text.
Effect 2 multiplies the current scale once on entering a down state and divides
once on leaving it, guarded by down_scaled. Exported scale effects center the
pivot while retaining pivotAsAnchor. Invalid runtime scale factors are rejected.
Rendering continues through existing dirty/appearance/transform synchronization.

Buttons bind linked_popup to an existing object handle at runtime. Input checks
the shared popup stack at pointer-down and records what that press closed, then
toggles the button's popup. A just-closed popup cannot reopen during that press.
Clicking inside a lower popup closes only the popups above it; clicking outside
closes all. Generic hide detaches and retains views for reuse. Linked windows use
the existing show/hide and modal registration APIs. Menus and combo dropdowns
participate in this same stack and closure bookkeeping; see popup-lifecycle.md.

Anchors use computed UI transforms and convert into root-local coordinates, so
nested/scaled UI uses the same coordinate space as the popup parent. Native Auto
placement can flip above and adjusts x near the right edge. A pending standalone
root adopted as a popup waits for its parent view and preserves its own package
record without reattaching as a stage root. The internal GRoot layout rectangle
is excluded from pointer targets (native opaque=false), so creating it cannot
occlude independent GUI roots. The top modal and popup layers above it may receive
input; layers below it remain blocked.

An executable fixture captures the unchanged official GButton class under a
minimal object/event host; it validates effects and pointer/selection/disabled
states, including no controller and changing scale while pressed. See
`docs/button-interaction.md` for source hash, capture command and scope.

The popup widget adapter opens combos on pointer-down and excludes editable input
children. Selection/reselection closes the full stack and publishes StateChanged
with pointer data. Menu row closing waits 100 ms and respects grayed/checked state.
Retained combo owners are checked before view synchronization; disposing an owner
reclaims its detached cached dropdown. Menu items populate the real list child.

Menu definitions carry stable names and optional action IDs. The adapter retains
each row's actual resource URL, so action and separator templates use separate
pool entries. Dynamic edits do not change the initial configuration snapshot;
reopening unchanged configuration preserves edited state and object/entity IDs.
The pending click stores its original pointer position. At expiry the adapter
validates current membership/visibility/enabled state, toggles the checked
controller, closes the menu, and queues GuiMenuAction for Publish. Respond can
consume it and reopen immediately. Removing/replacing/reopening/closing cancels
pending clicks, preventing pooled identities from executing stale actions.
List folding changes layout participation without changing logical row indices;
tree indentation still uses each row's original index. Popup fitting uses the
last participating row plus margins and is repeated on show.
GRoot's view defaults to GlobalZIndex(1), preserving the popup/window subtree's
render and picking order above standalone page roots created later. Explicit
application overrides remain supported; the root rectangle itself cannot hit.

## Dependency templates and resource provenance

`create_from_package_with_registry` recursively resolves child resource package
IDs through PackageRegistry. Instance properties still use the containing
component's string table; the child's source package owns its template and
descendants. Explicit foreign IDs never fall back to an equal local item ID.
Unresolved embedded references follow native GComponent's typed-object fallback.
An active resource stack rejects cyclic component references, and construction
errors remove partially created subtrees. AsyncBuilder exposes the same registry
path, which runtime deferred builds use.

GObject stores source_package_id independently of package_item_id and parent.
Runtime asset propagation changes handles at those package boundaries, preserving
explicit object provenance during reparenting and pending view construction.
Root, window, list, scrollbar and popup creation use registered dependencies.

The template resolver accepts local names and both native ui:// URL forms,
normalizing caches and pools to package/item IDs. Combo dropdown, list default
item and menu content/action/separator can each belong to a different package.
Replacements prepare rows before releasing usable content. Missing resources
return None and can be retried after registering the dependency.

Initial loader/icon rendering resolves a foreign URL's own asset before selecting
its atlas or movie data; dynamic relative paths use explicit object provenance
instead of searching standalone root parents. Explicit unresolved package URLs
stay empty rather than using an equal local name. Dirty sprite selection also
honors the object's package ID.

## Programmatic button feedback

ObjectStore owns fireClick visual callbacks with absolute f64 deadlines. Each
Common fire registers a Down and an Up callback without replacing earlier ones;
ready callbacks execute in insertion order. Subtree disposal removes its timers.
Input advances them before physical pointer routing, even with no window, so
Publish/Respond and normal view synchronization observe the current visual state.

GuiRuntime::fire_button_click immediately runs the internal button action and
bubbles StateChanged if selection changes. Native fireClick calls __click
directly; it does not dispatch Click, open a linked popup, operate the list/menu
adapter, or create pointer capture. Ordinary pointer Click/GuiEvent routing stays
independent. See button-interaction.md for API, timing and reference scope.

## Loader components

GuiRuntime records loader-owned component roots separately from static image/
movie content visuals. Model synchronization runs before view spawning, loads
components through the registry-aware construction path and records each source
asset boundary. Nested loader requests are processed in the same pass; ancestor
resource identities reject recursive expansion. URL replacement disposes the old
model/view subtree even if its replacement is missing. Hidden/detached owners
retain their content and asset handles.

The loaded root is a model child of its loader to represent the native display
relationship for stage, input, transforms and lifetime. Loader component layout
uses original resource dimensions and applies either scale or size, then position.
Owners are laid out in ancestor order so nested loader relations see parent size
changes. Images/movies remain in the existing content visual path; URL switching
removes those visuals before component view creation. See loader-components.md
for API, representation differences, native evidence and validation boundaries.

Loader status synchronization follows component construction and precedes view
spawning. It observes retained image handles and deduplicates movie atlas checks;
it does not restart failed requests during polling. Error signs use their own
canonical-URL FIFO pool, preserving object/view state and native failure-time
sizing. URL/state changes release the sign, while flag/config setters alone do
not. Explicit retries are consumed after the movie/component/status systems have
all seen them. Image storage is optional for input-only/headless plugin hosts;
image states remain Loading there until a rendering asset backend is available.

## Polygon material surfaces

The shared compositor replays these material draws together with ordinary UI
images, text and graph nodes. See [blend modes and isolated effects](compositing.md)
for batch barriers, filter isolation, inherited blend and view-target ownership.

Bevy 0.20.1 provides UiMaterial in bevy_ui_render. GGraph polygons use that normal
UI pipeline rather than a separate camera or world-space mesh. Core graph code
constructs native points and the closed stroke; a child surface carries the
material and storage buffer, with bounds expanded for stroke/outside vertices.
Its local negative ZIndex keeps drawing below object children. The graph's
authoritative view remains the entity returned by GuiRuntime.entity_of.

Polygon surfaces also carry GObjectRef for native object event routing. Systems
that need the authoritative view must compare entity_of(id) with the entity or
exclude PolygonSurface. Stage synchronization already makes this distinction;
gesture coordinates explicitly use the authoritative view. Surface picking
converts its enlarged local rectangle back into object space and rejects points
outside the contour. Generic appearance synchronization excludes surfaces so it
cannot tint a rectangular background behind the polygon material. The material
receives effective core alpha and colors directly.

Retained GraphRender state keeps the material and buffer handles stable during
updates. Geometry changes update buffer data, while ancestor opacity updates
only the material uniform. Shape switching destroys the surface and releases
strong handles; owner disposal follows normal recursive view cleanup. An
input-only application initializes the model/material asset types without
requiring RenderApp or a GPU. See polygon-rendering.md for evidence and limits.

Package handles are recorded independently of object parenting. Source propagation
respects explicit package boundaries, and each recursive spawn resolves that
object's source. Creating GRoot and a Basics dropdown together therefore cannot
borrow the first MainMenu package's atlas IDs. This covers initial, pending and
retained/reparented widget views, including resource-directory preservation.

## Image fill masks

FillUtils polygons describe cropping in the full image canvas, while layout,
pivot and pointer geometry retain the object's dimensions. The image-fill system
runs after ordinary appearance and movie/content layout synchronization. It keeps
ImageNode as the source descriptor, makes its ordinary draw transparent and
attaches a FillMaterial on the same entity. Generic appearance excludes active
FillMaterial nodes; the material obtains tint/effective alpha from the model.
Disabling fill removes that material and restores the normal image draw.
Native nine-grid images and images/movies with local color effects retain that
material with a full mask when fill is disabled; mapping and color effects are
independent of whether a fill mask is active.

The fill material keeps normalized atlas coordinates, nine-slice/tile mapping,
flip flags and a fixed-capacity array for the native mask polygon. Loader content
maps its trimmed node back into the aligned full content canvas; movie original
frame dimensions participate in that mapping. Loaded components/error signs use
their separate ownership paths and remain outside the content-image mask.
Material handles survive progress, texture and movie-frame changes. View disposal
releases them through normal Bevy asset tracking.

Core progress bindings capture bar/bar_v offsets and parent-to-bar size deltas
at construction. Updates choose fillAmount for filled Image/Loader bars and
axis-specific resizing otherwise; reverse, ani frames, title type and owner size
changes use the same binding. Instance initialization refreshes its serialized
value. Runtime percentages/fill amounts retain double precision until geometry
upload, preserving native title floor and near-complete mask thresholds. See
image-fill.md for APIs, reference fixtures and validation boundaries.

## Nine-grid texture mapping

An internal ImageGrid descriptor records original source dimensions, crop offset,
grid and the native normalized tiling flag. It is attached to direct exported
Image views and Loader ContentVisuals when their source is a grid image. Loader
sources also carry scale-by-tile metadata through initial and changed-URL paths;
foreign resources use their own package and atlas.

ImageGrid selects native axes/tile mapping in the existing FillMaterial, which
keeps its handle across mask/resize/alpha changes. The shader maps virtual
untrimmed source UVs, discards sampling outside the real atlas crop, repeats
edges on their long axis and repeats center on both axes. Grid axes independently
compress fixed borders when the target is too narrow/short. Corner/filter
behavior is no longer approximated by Bevy's shared corner-scale coefficient.
No grid-cell entity tree is required. The existing layout/input rectangles,
stage hierarchy and disposal remain authoritative. See nine-grid.md for native
flag normalization, source evidence, fixture generation and unverified scope.

## Local color matrices

ColorFilterParams interprets raw package/transition values in the native zero
default [-1,1] domain; the generic ColorMatrix convenience API keeps its separate
factor/degree domain. Native matrix construction uses double intermediates and
normalizes 0..255 offsets before upload. FillMaterial now also selects on local
filter/grayed for Image, standalone MovieClip, Loader image/movie visuals, bitmap
glyphs and rich-text inline images. TextTintSource resolves each sampled text
quad back to its model owner and supplies its raw foreground/white override.
Four matrix rows and one offset vector follow encoded base texture/tint
multiplication; the shader converts from/to Bevy's linear sample/output spaces.
Effective opacity is applied once. Identity effects need no material unless a
grid/mask already needs it. Materials survive effect/frame/texture updates and
return to ordinary ImageNode drawing when all three features are inactive.

Legacy ToolSet gray replaces the active matrix while retaining the latest stored
color parameters. Graph solid colors and text foregrounds use CPU matrix
application. Component/button grayed rules are now handled in the core before
appearance synchronization. Sampled text quads use their own full rectangles,
including glyphs extending beyond a small text owner. Default foreground becomes
#AAAAAA when gray; untinted bitmap glyphs, explicit spans and inline images keep
their respective overrides before filtering. Container/text offscreen composition
and PostProcess2D stacks remain separate work. Independent leaf filters can differ
from a composed-text filter when glyphs/images overlap or intermediate colors clamp.
See color-filters.md for reference execution, screenshot sampling and limits.

## Component and button grayed state

ObjectStore::set_grayed calls the native handler branch after assigning the flag.
Buttons with a disabled page drive disabled/selectedDisabled and restore up/down
on enable. Other components select index 1/0 on an internal grayed controller,
or assign each direct child's grayed flag recursively if no such controller
exists. Controlled/disabled skins do not receive an additional inherited matrix.

GObject keeps an internal first-assignment bit because native JS leaves _grayed
undefined. Public grayed() reads false initially, while the first false assignment
still runs handlers. Later equal assignments do not reapply child state. Adding
or reparenting a child preserves its independent flag. Touchable is independent.

Referenced components are fully built before their parent's instance properties
are parsed. A newly serialized true flag therefore runs its handler immediately
after setup_beforeAdd, before instance gears/selection and parent controller
initialization. Construction suppresses new controller gear tweens; existing
runtime controller setters handle subsequent changes. The behavior is shared by
synchronous, registry-backed and per-entry asynchronous construction.
See grayed-state.md for native traces, derived binary fixture and limits.

## Runtime gear capture

Each GObject carries gear_locked and under_construct independently. Property
setters call update_gear_state only where the reference setter calls updateGear.
Gear application/tween writes and Transition.applyValue/shake restoration lock
their target; forwarded writes on another object can still capture that object's
gears. Display gears retain their separate connection policy. Movie playback
reports update the live model without capturing a new saved animation state.

Gear setup seeds its default from pre-extension object properties, then reads
exported overrides. Resource title/icon caches are initialized from the built
compound control before a parent captures its instance gear defaults. Gear
application uses the same title/icon/font forwarding as normal property changes,
including empty titles. Captures save normalized model values on the bound
controller's current page. unselected_value holds null-page storage separately
from values[None], which remains the default fallback.

controller_owner records the original controller owner rather than re-resolving
controller_index against a new parent after reparenting. Current parent geometry
still supplies percent XY ratios. New-parent controller passes skip gears bound
elsewhere. The existing parent/sibling resize relation paths shift every saved
absolute XY/Size page/default and directly recapture the current page, following
GearXY/GearSize.updateFromRelations; percent XY skips the shift.
See gear-state.md for setter exceptions, fixture scope and remaining limits.

## Plain bitmap text layout

core::bitmap_text::layout_bitmap_text now computes the active runtime bitmap
path independently of Bevy text layout. It follows native BitmapFont/Text
measurement, per-glyph rounded advances, font autoScaleSize, two-pixel label
padding, native plain-text wrapping/alignment, Both/Height auto-size and
Shrink/Ellipsis. UTF-16 glyph lookup skips missing units without inventing a
fallback advance. Line height includes the native extra underline pixel.

Initial creation resolves a bitmap font even when there are no drawable glyphs.
BitmapGlyphs retains ownership and BitmapTextMetrics stores logical measured and
auto-sized bounds; empty/missing-only content does not fall back to Bevy Text.
sync_dirty rebuilds glyphs on TEXT or SIZE, using the same translated source as
initial creation and supporting the embedded text data of TextInput. Tint/alpha
continue through TextTintSource. Auto-size consumes bitmap metrics rather than
physical ComputedNode size, and preserves fixed Shrink/Ellipsis boxes.

Image-backed glyph quads map their full logical canvas through source texture
trim offsets/actual atlas crop. Font parsing reads logical branch dimensions
before resolving an HD texture. Atlas-backed glyphs retain their font subtexture.
No new generic offscreen pass is introduced. Public legacy text::layout_line and
layout_text remain simple helpers; they are no longer the runtime layout path.
See bitmap-text.md for executable native evidence, fixtures and remaining scope.

## Text font selection and renderer replacement

runtime::text_fonts::spawn_text is shared by first creation and renderer changes.
TextFontBinding records the requested font, selected package/item/res_key, UBB
mode and configured ordinary font. sync_text_fonts rechecks selection after
sync_dirty and before rich span synchronization. Bare names are local; explicit
URLs select their declared loaded FuiAsset. Every glyph texture index/path is
interpreted in the font source package, independently of the text object's asset.
Source arrival/removal changes the selected backend even without a new dirty
flag; a newer request takes precedence over a late source.

Switches retain the object/view, unrelated children, model size/layout ownership
and listeners. Only tracked glyphs, rich spans and linked image siblings are
despawned. Switching removes the old Bevy Text required components as well as
ComputedTextBlock, TextLayoutInfo and ContentSize, and clears alignment padding.
Ordinary text restoration reintroduces paragraph components; UBB restoration
recreates spans/images with correct sibling ownership. Ordinary default-font
changes update TextFont without resetting EditableText. Input bitmap caret,
HTML form-object rendering, branch/HD selection
changes and in-place font hot reload remain outside this path's verified scope.
Ordinary family/file selection now follows the source policy described in
docs/raster-fonts.md. Input creation initializes the actual editor buffer, and
InputTextState distinguishes model/translation updates from committed editor
edits. The Text component mirrors presentation (the committed value for ordinary
fields, the mask for passwords); it is not the authoritative editing buffer.
Font changes preserve the editor and queued edits.

InputEditPolicy compiles native ECMAScript restriction syntax once per setting
change and tracks UTF-16 edit limits, multiline mode and asynchronous clipboard
state. GuiSystems::Input synchronizes current model/policy settings before early
editing and pointer focus changes; PostUpdate handles late queues after editor
style/viewport sync and before Bevy EditableTextSystems. It clips user edits, validates
whole committed values, preserves preedit, and maintains paste FIFO ordering.
Other editors continue through Bevy's normal system. Programmatic model values
bypass restriction/max length. See docs/text-input.md for reference scope and
remaining input-event/DOM behavior.

Ordinary input focus, KeyboardInput and Ime routing use the same GUI hit/modal
policy as pointers. The bridge observes FocusedInput events without adding Bevy's
TextInput marker, avoiding duplicate upstream widget handlers. Session focus is
reconciled before dispatch, after pointer routing and after synchronization;
invalid/hidden/off-stage/window-blurred inputs cancel preedit/paste and release
focus. GuiInputState exposes keyboard ownership. GuiTextInput publishes target-
only INPUT/ENTER/CHANGE/FOCUS/BLUR equivalents; submit default completion waits
until Respond can cancel it or transfer focus. Real committed edits update the
model and InputTextState before publication. Window IME state/candidate geometry
follows the selected GUI window and masked geometry for passwords. See
docs/text-input.md for native trace coverage, callback timing and open boundaries.

TextCursorStyle activates Bevy caret/selection drawing on ordinary input owners.
Its foreground/opacity follow the effective TextColor; cursor width resolves to
one logical pixel using the target scale and evaluated font size. The component
is removed when switching away from the editable renderer. Masked inputs reuse
the owner's cursor paint with projected geometry; layout-only children never
become separate visible inputs.

UBB bitmap fields use core::bitmap_text::layout_bitmap_rich_text, whose explicit
elements model native text/image/link commands and wrapping across style
boundaries. The adapter tracks glyphs separately from its image/decoration/
generation children; all are inside the same retained text owner. It snapshots
source/properties/size/image identity and rebuilds after font selection sync,
including late foreign images. Source images and glyphs use sampled matrices;
TextDecoration backgrounds use CPU appearance. Input accepts bitmap native link
areas without requiring a Bevy ComputedTextBlock. See bitmap-rich-text.md for
reference execution, ownership, tested boundaries and remaining gaps.


## Input prompt and read-only views

Input prompts have independent owned child views, tagged InputPromptOwner.
Their snapshot excludes live editor text, so typing only changes visibility.
Prompt source/style/font/size/translation changes rebuild that child without
touching the editor or application children. Ordinary prompts use the selected
FontSource; bitmap prompts use package glyph layout/tint semantics. UBB removal
precedes translation and does not run color handlers. A post-edit visibility pass
precedes VisibilityPropagate; it is ordered before EditableTextSystems rather
than after UI PostLayout, because menu focus systems require visibility
propagation before PostLayout. Read-only policy maps TextReadWriteMode and rejects
queued mutations while preserving navigation/copy and programmatic changes.
See docs/text-input.md for evidence and remaining prompt layout/input scope.

Password inputs retain their real EditableText editor and core value. An owned
layout-only child (no Node/Text/GObjectRef) uses a second editor containing only
the UTF-16 mask. After input editing, a cached boundary map projects selection
and IME ranges into that editor. Bevy computes its font/atlas geometry; after
update_editable_text_layout, publish_password_layout replaces all owner draw
geometry and viewport offsets with the masked result, retaining the owner's
cursor blink state. Pointer/scroll actions map through the mask before actual
source edits. Disabling the mode invalidates raw layout without replacing the
editor. Font/renderer switches and disposal reclaim the projection child while
preserving application children. Rich/bitmap sources receive masked text before
their existing renderers; their interactive caret remains outside verified scope.
See docs/text-input.md for clipboard policy, native evidence and boundaries.

## Vector text stroke rendering

text_stroke observes actual Parley glyph runs after ordinary/editable text layout
and password geometry publication. It rasterizes round-join vector strokes with
Swash/Zeno, caching separate FontAtlas pages per font/size/variation/width/hinting/
smoothing. Explicit span overrides inherit current field defaults. The owner
receives TextStrokeGeometry; no stroke UI children or second layout are created.
Geometry changes invalidate normal UI extraction through TextLayoutInfo. The
text_composite module combines stroke/fill masks per line/section in encoded
channels, then applies local filters before drawing effective object opacity once.
Cached RGBA tiles replace only newly extracted foreground batches, preserving
shadows, decorations and uncovered spans. Active custom selected-text colors keep
the normal foreground path to preserve their already adjusted opacity. The render
pass uses the same transforms,
camera, clip, content origin and input viewport offsets. Opacity-only changes
retain images; source/style/filter/selection edits invalidate them. Tiles include
neighbor borders, and isolated atlas sampling retains the font sampler. Stable
geometry retains buffers;
unused atlas keys are released after style/backend switches or disposal. Bitmap
backends bypass strokes as native image glyph commands do. See text-stroke.md for
native command evidence, validation and remaining compositing/rasterization limits.


## Materialized list selection

core::object::list_selection uses row buttons as the selection authority, with
ListData snapshots refreshed after selection and child-lifecycle changes. Bulk
operations suppress intermediate snapshot scans. Attaching a row clears selected
and disables the button's own click toggle. Selection operations reuse button
visual/title/icon and controller code. Parent selection-controller updates are
bidirectional and guarded against collapsing the originating multiple selection;
initialization after row materialization restores the exported controller page
without rerunning controller actions.

The pointer router passes release-time Shift/Ctrl state and resolves nested row
content. Normal row clicks publish target-only GuiListItemClick before Respond;
menu/combo rows retain their specialized action routes. See list-selection.md for
native evidence, operation policy and virtual/tree/navigation boundaries.


## Scroll-to-view commands

core::object::scroll_view routes content rectangles, object bounds and materialized
list rows to ScrollPane's native reveal policy. Direct child bounds bypass their
own transform; nested rectangles use two-corner parent-to-owner conversion with
model transforms, margins and displayed scroll offsets. A list's explicit command
may use its parent's pane, while click-triggered reveal uses only the list's own
pane and respects its exported/runtime flag independently of selection mode.

ScrollPane retains a logical requested position and a separate display position
for programmatic cubic-out tweens. Dirty scroll projection, scrollbar grips and
virtual-list ranges use display position. Repeated target requests retain tween
progress; retargets use current display coordinates. Grip/track handling likewise
uses displayed percentages. Core fixtures and adapter tests run without creating
native windows. See scroll-to-view.md for native method evidence and remaining
virtual/snap/page/event/renderer limits.

Registered pane bars stay in ObjectStore's ownership tree for disposal while
list_items/list_item_at/list_item_index expose the content-only sequence. List
layout, selection, reveal, recycling and popup replacement use this sequence.
The adapter marks bar roots with IgnoreScroll and removes its own marker when
they detach, so retained bar entities can be reused safely. See list-scrollbars.md.

Scrollbar press policy lives in core::object::scroll_bar_input. Registered nested
skin hits resolve to one pane bar. The adapter invokes the core policy once on
MouseDown, bounds propagation at grip/arrow parts and preserves a separate
captured grip drag. Release dispatches ordinary click notifications without
repeating the scroll action or applying list-row behavior. Track direction uses
grip-local coordinates through the existing model point/rectangle conversion.

Each pane axis owns an independent visibility/alpha/grip state. Activity
transitions refresh the native policy; the idle fade uses delayed QuadOut timing
and restores alpha before hiding. Captured grip lifetime is validated against
registered bar identity, with cleanup on release and invalidation. ObjectStore
projects visibility and alpha to bar roots, keeping ordinary hit eligibility and
inherited image opacity in sync. See scrollbar-visibility.md for evidence/limits.

core::object::scroll_wheel walks the actual hit's ancestor path, applying native
scalar wheel input to every pane and marking each changed owner for projection.
The adapter maps Bevy line input to native sign/step policy and retains precise
pixel input as an explicit backend extension. ScrollPane owns enable/step settings
and ceiled, direction-filtered overlap bounds; arrow/track actions reuse its step.
See scroll-wheel.md for native source provenance, event contracts and limits.

core::object::scroll_pages retains page-controller bindings by pane and controller
owner/index, independently of GList selection-controller binding. Controller
application visits bound direct children; pane feedback suppresses only its
source while forwarding gears, selection and actions normally. Exported bindings
wait for the adapter's shared root/subtree preparation to finish materialized
rows and viewport layout before applying the home page once. See scroll-pages.md.
Each rendered pane also owns an ECS-only hidden/unpickable scroll extent. It
keeps Bevy layout's clamp range at least as large as the authoritative model's
overlap, including empty page tails. It has no logical object/list index and
shares the owner's view lifetime; sync_scroll_extents follows model range changes.

ScrollPane's native command path aligns page/item requests before committing
logical targets and type-1 tweens. ObjectStore refreshes materialized snap
geometry from current content children at each command. Raw position/bounce APIs
and smooth Pixel input remain explicit extensions. Native per-axis tween clamps
preserve cleared changes for killTween semantics. See scroll-alignment.md.

Accepted content drags use a separate core::scroll::motion state. Native filtered
velocity and screen thresholds plan independent type-2 release axes, release
page/item alignment, shortened durations and edge-return legs. Logical positions
follow displayed movement for controller feedback. Store release refreshes current
snap geometry; adapter capture supplies scale, stationary time and release delay.
Post-layout project_scroll_overflow restores displacement removed by Bevy's clamp,
shifting logical content paint descendants through the pane transform before text
and clipping while preserving pane bars and layout bounds. See scroll-motion.md.

core::scroll::refresh adds resource maxima, locked bounds and header/footer display
geometry within the existing boxed motion state. core::object::scroll_refresh
owns resource creation, content exclusion, stage connection, constrained sizes,
queued releases and lock commands. A pending release permits response-time locks
before committing the native return target. The adapter publishes target-only
GuiPullRelease messages before Respond and finalizes in Advance. Refresh views
share pane ownership while native stage membership follows actual reveal, keeping
auto-transition start/stop separate from authored visibility. See pull-refresh.md.
