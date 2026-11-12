# Stage and transition lifecycle

`ObjectStore::is_on_stage` reports display-tree connection. The external adapter
connects roots using `attach_to_stage` and `detach_from_stage`. `set_root` remains
a convenience root identity; it does not connect a host display tree. Attaching
an external root first removes any logical parent; adopting it into a logical
parent consumes that external-root attachment.

Children inherit connection through their parent and display gears/groups.
Explicit `visible=false`, alpha, or scroll clipping do not disconnect sprites.
Display/Display2 disconnects do; active display locks can keep them connected.
Group objects have no sprite themselves, but their connection affects members.
Applying one controller's gears to an object batches connection changes until
all its gears have applied, matching GObject.handleControllerChanged.
Transition locks are acquired after initial item playback and retained across
repeats. This prevents an initial lock from starting child auto-play before a
nested call, or a loop boundary from disconnecting and reconnecting a child.

| Change | Transition behavior |
| --- | --- |
| Enter stage | Start auto-play only if it is not already playing |
| Leave, default options | Stop, retaining current values and restoring Shake/locks |
| Leave, option 4 | Complete pending values, then stop |
| Leave, option 2 or 6 | Continue; disabled auto-stop has priority |
| Dispose | Cancel even when auto-stop is disabled |
| Reorder within same parent | Preserve playing state and progress |
| Reparent between staged parents | Leave then enter; stopped auto-play starts again |

Stage stops do not process the play's completion callback. Item hooks produced by
complete-stop still follow the existing hook rules. A waiting parent receives no
child completion from a stage stop, matching the official processCallback=false.

`set_transition_auto_play` defaults to `times=-1`, `delay=0`. The options variant
accepts explicit values. Equal enabled flags leave the previous settings unchanged.
Enabling onstage replaces an existing play; enabling offstage just configures.
Disabling onstage leaves the current play running, including its current repeat
count; disabling offstage stops with current values and processes completion.

`GuiRuntime::create_root` builds the model; it joins the stage after the view and
asset are ready. `set_root_attached(index, false)` hides/unmounts that tree while
retaining its entities. Set true to reattach and apply the rules above. The flag
can be configured before spawning. Core custom adapters that manually create
Bevy views must explicitly attach their host roots.

The view synchronizer updates ChildOf and inherited visibility from the logical
tree. Detach preserves objects/entities for pooling. Remove disposes and cleans
the subtree's entity mappings; surviving detached children are unparented before
removed parents are despawned. Offstage objects cannot receive pointer input.
Explicit removal methods repair surviving parents before queuing recursive
despawn too, covering same-frame detach/pool-and-remove. Menu and dropdown pools
retain their item view subtrees rather than keeping stale descendant mappings.

Ordinary MovieClip frame updates require stage connection. Explicitly hidden
movies keep advancing, while leaving the stage preserves their fractional frame
time until reentry. DeltaTime/frame/play-settings commands still apply offstage.

Window hide retains its subtree; `show_existing_window` reuses it. Close disposes
and cleans mappings for all descendants. Bring-to-front uses child reordering
without a stage exit/entry pair.

The checked-in native reference trace records results from the original Transition/tween
classes with host objects and validates auto-play, all removal-stop flag branches,
reentry and disposal. This verifies the transition rules, not the full Laya stage
event dispatcher. Core tests cover tree/gear/group propagation and Bevy tests
cover hierarchy/visibility, movie timers, input and cleanup. Window smoke checks
auto-play delay, unmount stop, a hidden retained view, remount on the same entity,
onstage auto-play disabling and final disposal.

UBB inline images retain sibling entities for layout and separately link to their
text owner with InlineImageOwner/InlineImageParts. Reparenting moves both views;
pooling hides and retains them; owner disposal reclaims all owned images. Explicit
same-frame removal protects these siblings along with surviving mapped objects.
See [rich-text ownership](rich-text-ownership.md) for details and validation scope.
