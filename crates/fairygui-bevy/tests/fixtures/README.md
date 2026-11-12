# Executable MovieClip reference traces

`movie-reference.txt` is generated from the unchanged `MovieClip` class in the
official [FairyGUI-layabox](https://github.com/fairygui/FairyGUI-layabox)
`source/bin/fairygui.js` distribution (MIT). Its header records a SHA256 of the
extracted class. The fixture captures frames, direction, repetition, internal
status, elapsed milliseconds, Playing, timeScale, remaining plays and callback
counts after each command.

Reference checkout used for this capture:
`0a9368b7873e82bfe372323e0059685ee443bef8`. The class SHA256 is
`a5b74da77c759ecb3ee868c0903cdaabe27126fc4bc24016c0d3747fa10bbb6f`.

The scalar capture host stubs only Image drawing/event registration and Laya's timer
host. MovieClip's update/advance/setPlaySettings/frame/rewind methods run unchanged.
The fixture includes fixed boundary cases and deterministic mixed command streams.
It tests playback state, not rendering or native frame scheduling.

The checked-in trace is consumed directly by Rust tests:

```text
cargo test -p fairygui-bevy executable_official_movie_traces --offline
```

Tests consume the checked-in trace and do not require Node or the reference clone.
The runtime also guards malformed all-zero-duration DeltaTime cycles and clamps
out-of-range range endpoints; those defensive cases do not claim reference parity.

## Text image color traces

`text-image-color-reference.txt` records 4 source tint policies, 28 tint/matrix
states and 84 straight RGBA samples. It records unchanged type-stripped Laya 3.3
Text.renderText, FairyGUI GObject grayed
accessors/handler, GTextField color/grayed methods, ToolSet.setColorFilter and
ColorMatrix. Fixture headers record source method hashes.

The minimal host records drawImage's selected tint and ColorFilter's chosen
matrix; GTextField's display.color updates Text's default style. Cases include
tintable/colored glyphs, explicit colored spans and a representative white image
source. The last case does not execute HtmlImage: actual exported inline-image
ownership and sampled materials are covered by adapter tests and Windows GPU
smoke. RGBA arithmetic uses the captured tint/matrix, without native GPU drawing.
This evidence covers local sample colors, not filtering overlapping composed text.

## Stroke/fill composition traces

`text-composite-reference.txt` records 288 results from unchanged Laya 3.3
TextRender.drawOffscreen under a numerical Canvas host. Known stroke/fill coverage and
paint alpha values exercise source-over call order, including disabled strokes
and transparent paints. RGB uses encoded-channel arithmetic. The source method
hash is in the header. This does not capture browser glyph rasterization or a
native GPU framebuffer; independent Bevy GPU captures validate runtime drawing.
