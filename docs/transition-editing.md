# Transition editing

The core exposes typed versions of the FairyGUI Transition editing methods:

| API | Matching and timing |
| --- | --- |
| `set_transition_value` | All matching start/end labels; start wins within an item |
| `set_transition_duration` | All matching tween start labels; exported total duration stays fixed |
| `set_transition_target` | All matching start labels; None means the transition owner |
| `transition_label_time` | First matching start/end label, original forward timeline |
| `set_transition_hook` | First matching start/end label, one optional listener id |
| `clear_transition_hooks` | Clears user listener bindings, preserving automatic label notifications |

The `Transition` type also provides these methods directly. Store wrappers return
`TransitionEditError` for missing definitions/labels, incompatible values, invalid
numeric input, stale targets or active-driver reentry. Value updates validate all
matches before changing any of them. They avoid the reference's unchecked JS
coercion and partial invalid writes.

```rust,ignore
use fairygui_core::transition::TransitionEditValue;

runtime.store_mut().set_transition_value(owner, "t0", "end",
    TransitionEditValue::Pair { x: 60.0, y: -100.0 })?;
runtime.store_mut().set_transition_hook(owner, "t0", "play_num_now", Some(42))?;
```

Read `GuiTransitionCallback` messages and route `callback.0.listener`. Existing
`GuiTransitionHook` consumers continue receiving automatic label messages.
Changing a hook affects subsequent callbacks; already queued notifications remain.

Tween duration and numeric endpoints are copied when a pass initializes, including
pending tweens. Editing them takes effect on the next pass/replay; editing during
startup delay affects the first pass. On tween start, missing axes and child
percentage positions use live definitions and object geometry, as in the official
`onTweenStart`. Retargeting a created tween changes its recipient while retaining
its interpolation. A missing/foreign target skipped during creation cannot gain a
task until replay. Only direct children and the owner receive values.

Future discrete actions read current values. Nested transition names remain bound
for a play operation, whereas their play count is read at the cue. Sound items
retain their first-triggered address cache across edits/replay; volume remains
live. A null Text/Icon value clears its content. Relation layout movement shifts
absolute XY definitions, including both endpoints, and leaves percentages alone.

`setDuration` does not recompute nominal total duration. A subsequent reverse
play may therefore schedule an edited longer tween before zero and begin inside
it. Label-time queries still return original forward cue time plus the edited
nominal tween duration, ignoring repeats. Transition `times=0` plays once; negative
counts are infinite. MovieClip uses a different zero-count rule.

The checked-in native reference trace records results from the unchanged distributed
Transition, GTween, GTweener, TweenManager and easing classes with object/timer
hosts. It checks the behaviors above against the official checkout
`0a9368b7873e82bfe372323e0059685ee443bef8`; its log includes a Transition SHA256.
Run it with Node from the workspace root. Rust regressions check production
behavior, and window smoke edits real PowerUp labels, retargets its numeric text,
and checks the resulting node position and callback routing. These are bounded
behavior checks, not a complete native rendering/lifecycle comparison.
