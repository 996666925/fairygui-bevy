# MovieClip timing reference

The runtime keeps ordinary frame updates separate from explicit DeltaTime
catch-up, following FairyGUI-layabox's `MovieClip.ts` and its distributed JS class.

Ordinary updates clamp host time to 100ms **before** applying timeScale and take
at most one frame step. The leftover is capped to the base frame interval.
Looping/end states then run after that step. This includes an observable detail:
the default full-clip loop restarts at frame zero after showing its last frame,
overriding the swing step. Explicit `advance` catch-up instead walks swing frames
without applying range completion states.

Finite ranges enter an ending state when they reach `end`. That frame remains
visible until the next update satisfies that frame's duration, including additional
delay and retained elapsed time, before playback moves to `end_at` and invokes
completion. Playing remains true if it was true. Neither
frame assignment nor Playing toggles nor rewind clears the ended state; applying
new settings does. Assigning the same frame retains fractional time.

Catch-up ignores Playing, timeScale and range status, discards fractional elapsed
time, and uses the original first-return cycle calculation. In particular, its
first-loop optimization can produce a different result from mathematically
skipping steady-state cycles when repeatDelay is present. The executable fixture
records this branch rather than assuming both algorithms are interchangeable.

The [fixture README](../crates/fairygui-bevy/tests/fixtures/README.md) documents
the source and trace scope. The checked-in native reference trace records results from the
unchanged distributed class with only the Image host and Laya timer registration
stubbed. Tests replay more than 1,200 commands and compare frame, direction,
repetition, elapsed time, range status, speed, remaining plays and completion
counts after each command. This is playback-state evidence, not a renderer or
native timer integration comparison.

The Rust adapter additionally clamps invalid frame endpoints, rejects nonfinite
or negative elapsed time and invalid settings, and terminates malformed
zero-duration catch-up cycles. These defensive input cases do not claim parity
with the reference's unchecked behavior.
