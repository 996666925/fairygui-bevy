# 测试夹具

本目录下的 `.fui` 文件是 **FairyGUI 官方示例工程**的导出产物，取自
[FairyGUI-layabox](https://github.com/fairygui/FairyGUI-layabox) 仓库的
`demo/assets/resources/ui/`，遵循其 MIT 许可。

| 文件 | 用途 |
| --- | --- |
| `Joystick.fui` | 最小规模的包，用于验证最小输入下的解析 |
| `MainMenu.fui` | 组件、图片、九宫格与图集 |
| `Bag.fui` | 序列帧（影片剪辑） |
| `Transition.fui` | 条目数量较多，用于验证条目表与精灵表的规模 |
| `Basics.fui` | 控制器、齿轮、按钮与可滚动容器 |
| `Chat.fui` | 唯一配置了滚动条的示例包，用于验证滚动条的实例化与联动 |
| `Cooldown.fui` | 径向填充进度条的初始化、实际取值与稳定图片尺寸 |

以上 `.fui` 文件用于核心运行时测试，不参与运行时的资源加载；完整工程请见上游仓库。

`PopupHost.fui`、`PopupTheme.fui`、`PopupCycle.fui` 是基于上述 `Basics.fui`
生成的依赖测试夹具。运行 `python tools/generate_popup_packages.py` 可确定性重建。
脚本只改包身份、依赖和选定资源引用，保留原组件记录、控制器和关联关系。
Host 下拉框引用 Theme 的弹层，Theme 的列表项再引用 Host；两包刻意保留相同
条目 ID，以验证跨包引用不会误用本包同名 ID。Theme 还含跨包子组件及图片引用，
Cycle 含循环组件引用，用于验证构造失败后回收部分对象树。
Windows smoke 直接加载 Host/Theme 夹具并共用官方 Basics 图集，不另存图片副本。

之所以直接使用真实导出产物而非程序化构造的二进制，是因为包格式含有若干
`Int16`/`Uint16`/`Int32` 混用的偏移字段与两段式字符串编码，只有真实数据才能
覆盖这些分支。

`tree-settings-reference.txt` runs the unchanged GTree indent/clickToExpand
setters, createCell and dispatchItemEvent methods with scalar cell/event hosts.
Its source hash covers all extracted methods. The 27 indent cases distinguish
retained cells from newly constructed cells; 20 click cases cover policies
0, 1, 2 and other nonzero values on folders and leaves.

## Executable GButton reference

`button-reference.txt` executes the unchanged GButton class extracted from
`.refs/FairyGUI-layabox/source/bin/fairygui.js` (MIT). Its header records the class
SHA256. The capture supplies only a minimal GComponent/text/event host, with a
live display-object sentinel so the native mouse-up disposal guard stays open.
The captured GButton state/effect methods run unchanged.

Core tests consume the checked-in trace without Node or the reference checkout. See
`docs/button-interaction.md` at the repository root for scope and validation.

`button-fire-reference.txt` separately executes the unchanged native fireClick,
internal click and visual-state methods with a deterministic one-shot timer host.
The source hash
matches the original button fixture; the host is explicit about registration
order and does not claim to reproduce Laya's frame scheduler. It covers 110
commands across 14 cases, including immediate actions versus delayed visuals and
the absence of a dispatched Click event.

`loader-component-reference.txt` executes unchanged native GLoader.updateLayout
with a minimal component size/scale host. Its 42 cases
and 70 layouts cover all fill modes, shrink-only, alignment, auto-size including
zero dimensions, resize versus scale and switching between modes. The header
records the unchanged GLoader source hash. See `docs/loader-components.md`.

`loader-error-reference.txt` executes unchanged GLoader error methods and the
unchanged GObjectPool with minimal display/package hosts. Its 30 steps record sign size,
show/config changes, canonical URL aliases, FIFO object identity, preserved title,
display attachment and pool count. Both class hashes are recorded in the header.
Bevy adapter tests compare this lifecycle trace; native rendering and asynchronous
asset requests are outside the fixture host.

`polygon-reference.txt` runs unchanged GGraph drawing/point generation, LayaAir
3.3 BasePoly.createLine2 and Utils.testPointInPolygon. The source methods are
described in `docs/polygon-rendering.md`. Node's built-in TypeScript stripping removes
types/imports without rewriting BasePoly control flow. The trace records source
hashes, 21 sets of points/stroke geometry and 756 native contour hits. It checks
geometry/input semantics, not native rasterization or GPU output.

`fill-reference.txt` runs unchanged bundled FillUtils and records 1,680 masks
across five methods, all origins/directions, non-square sizes and boundary
amounts, including Float32 versus JS Number at the near-complete cutoff.
Rust tests consume the checked-in reference trace directly.
`progress-reference.txt` runs unchanged GProgressBar.update/setFillAmount under
minimal Image/Loader/size/text hosts. Its 24 cases/144 updates cover horizontal
and vertical bars, fill/non-fill paths, reverse, owner resize, ani frames and all
title formats (including double-precision floor behavior). Both fixture headers record source
hashes; they do not reproduce native rendering or Laya scheduling.

`nine-grid-reference.txt` executes unchanged type-stripped LayaAir 3.3
MeshFactory and bundled FairyGUI Image.doDraw. It captures 140 target/UV layouts,
688 nonempty tiled/stretch quads and eight tileGridIndice flag conversions.
The fixture records both source hashes and compares native geometry/UVs, not
native GPU sampling or clipping. See `docs/nine-grid.md` for the source scope.

`NineGrid.fui` is deterministically derived from the official Basics export by
`python tools/generate_nine_grid_package.py`. Only package identity/URL IDs,
dependency metadata and nonzero tileGridIndice on 34 grid images change.
Component records and atlas paths remain native. Smoke reuses the official
Basics atlas and additionally supplies a clearly patterned test texture to
distinguish stretch/repeat, tile tails and border compression.

`color-filter-reference.txt` executes unchanged native FairyGUI-Laya ColorMatrix
and ToolSet.setColorFilter under a minimal legacy ColorFilter matrix host, using
LayaAir 3.3's gray constant. Source hashes, 48 native matrices,
336 RGBA samples and five gray/filter override/restore states are recorded.
It covers defaults, individual/combined/clamped parameters, NaN/infinity, offsets,
alpha preservation and the distinction between saturation -1 and gray. See
`docs/color-filters.md` for input locations, GPU checks and remaining scope.

`grayed-reference.txt` executes native grayed accessors/handlers, GButton.setState,
Controller and immediate GearLook.apply under a minimal tree/controller host.
Its 27 cases/76 operations record 273 object states, covering recursive component
assignment, controller/disabled-page precedence, first false, same-value no-ops,
derived widgets and late/reparented children. Rendering, controller events and native
per-page runtime gear-value recapture are outside the host.

`Grayed.fui` derives from Basics via `python tools/generate_grayed_package.py`.
Only identity/URL IDs, dependencies and 22 referenced component instance gray
flags change. Native controllers, gears, selection records and atlases are
preserved. It exercises nested Image/Text gray and the existing Button52 grayed
controller and Button82 disabled/selectedDisabled skins. See docs/grayed-state.md.

`gear-updates-reference.txt` executes native GObject/control setters, GearBase,
eight value gears, Controller and Transition.applyValue under a minimal display
and tree host. Its 40 cases/279 operations compare 1,674 live/default/page-storage
values, including null-page separation, setter exceptions, locks/construction,
percent XY, retained controller binding after reparenting, relation shifts,
partial tween edits/retargeting and transitions. Explicit interpolation inputs
exercise native tween callbacks; the host does not reproduce native GTween
scheduling, a relation solver or rendering. See docs/gear-state.md.

`bitmap-text-reference.txt` executes unchanged type-stripped LayaAir 3.3
BitmapFont, Text.doLayout/renderText and their helpers with a minimal draw host.
Its 422 layouts and 2,391 glyph rectangles cover scaling/rounding, missing UTF-16
glyphs, plain wrapping, words/punctuation, alignment, all auto-size modes and
seeded boundary combinations. Source hashes are recorded;
native GPU rasterization and rich/HTML command combinations are outside the host.

`BitmapText.fui` derives from Basics using
`python tools/generate_bitmap_text_package.py`. Identity/URL/dependency data,
font auto-scale headers and nine Demo_Text field properties change; native font
glyph records and atlas pixels are retained. It exercises both image-backed and
font-atlas glyphs, resize/auto-size and empty/missing content. See docs/bitmap-text.md.

`bitmap-rich-reference.txt` executes the same unchanged type-stripped BitmapFont,
Text.doLayout/renderText and helpers with explicit styled text/image/link
elements. The host records image positions, link areas and glyph draws;
native HTML/UBB parsing, asset loading and native GPU drawing are outside it.
Rust tests consume the checked-in reference trace directly.
It records **955 layouts, 6,208 glyphs, 406 image positions and 287 link areas**,
including mixed sizes, command-spanning words/punctuation, fixed/scalable fonts,
all auto-size modes, images, adjacent links, image-only ellipsis, empty links
and seeded mixed commands. Source hashes are in the fixture header. Native
undefined image-only ellipsis measurements can produce NaN command widths;
geometry tests retain these as well as the resulting finite max-line metrics.
See docs/bitmap-rich-text.md for adapter ownership, input tests and render limits.

`markup-reference.txt` executes unchanged type-stripped LayaAir 3.3 UBBParser,
HtmlParser, XMLIterator, XMLUtils, HtmlElement, HtmlParseOptions and TextStyle,
plus ColorUtils/Color for markup color samples. It records **783 parser cases,
1,851 elements and 13 color samples**. Each case compares UBB-generated HTML,
element kind/text/style boundaries, XML attributes and select item/value arrays.
Coverage includes mixed nesting/mismatched closes, case, escaped brackets,
unknown/malformed tags, relative sizes, link defaults/explicit color precedence,
entities, images, whitespace/paragraph handling, skipped text and seeded cases.

Source file hashes are recorded in the header. Inputs and strings use hex-encoded UTF-8 fields to retain
newlines, empty strings and quotes without textual escaping ambiguity. The host
substitutes registration/image/link classes and allocates fresh HtmlElements, so
native image loading, reused pool state, rendering and Unicode unpaired-surrogate
storage are outside this evidence. See docs/markup.md for rendering and limits.

`inline-image-reference.txt` executes unchanged type-stripped LayaAir 3.3
HtmlImage and XMLUtils with minimal Sprite/Texture/owner/cache/callback hosts.
Its **33 cases, 66 dimension states and 3 stale-callback states** cover cached,
deferred and failed textures, independent explicit/automatic axes, zero/-1,
percent/invalid/hex/negative attributes, replacement, release and destruction.
Source hashes are in the header. Core compares dimensions; callback rows record native behavior,
with actual obsolete-result/disposal handling exercised separately in Bevy tests.
The host does not emulate native Sprite auto-size internals, pooled sizing
transients, AtlasResource animation or GPU drawing. See docs/external-inline-images.md.

`bitmap-decoration-reference.txt` uses the same unchanged native Text.doLayout,
Text.renderText and BitmapFont host as bitmap-text-reference, with native
underline and strike flags enabled. It records **422
layouts, 2,391 glyphs, 757 underlines and 757 strikes**, including wrapping,
alignment, auto-size, shrink, ellipsis, missing glyphs and empty lines. drawLine
is recorded as its horizontal stroke bounds (center y minus half thickness).
Core checks actual plain-layout underlines and the shared strike-bound helper.
Native GPU antialiasing and object-command pooled fontSize behavior are outside
this fixture. The default generator still reproduces bitmap-text-reference.

`font-selection-reference.txt` executes unchanged native LayaAir 3.3 Text.font
and Utils.getFileExtension under minimal config/cache/loader/BitmapFont hosts.
It contains
**25 classifications and 7 lifecycle states**: ordinary names, extensions and
queries, res://, native last-dot directory behavior, cached/deferred/failure/
obsolete callbacks and registered/cached/deferred bitmap selection. Core compares
classification; lifecycle rows document source behavior while Bevy adapter tests
exercise actual selection/lifecycle. Native browser font discovery, font decoding,
font-loader scheduling and GPU rendering are outside the host. See docs/raster-fonts.md.

`input-reference.txt` executes unchanged type-stripped LayaAir 3.3 Input.text and
TextInputAdapter restriction compilation/validation, plus its DOM maxLength
assignment expression. It records **576 validation states, 4 invalid restrictions, 24 programmatic
assignments and 6 maxLength properties**. Core compares ECMAScript/UTF-16 range,
escape/exclusion, whitespace, newline, non-Latin/astral and malformed-class states.
Source hashes are in the header. Minimal Text/PAL hosts do not emulate browser
insertion/selection, IME, DOM maxLength enforcement or OS input. See docs/text-input.md.


`input-prompt-reference.txt` executes unchanged UBBParser, GTextInput.promptText,
Text._typeset and Input.editable accessors, using minimal Text/HtmlElement/PAL
hosts. Its **24
UBB prompt cases, 96 projections and 4 editable states** cover known/unknown tags,
escaped brackets, retained image sources, literal HTML, CRLF, translation, empty/
nonempty/hidden text, default prompt color and editable defaults. Core compares
removal/projection invariants; Bevy tests cover actual ownership/editing. Native
DOM placeholders, HTML styling, line measurements, drawing and focus are outside
this host. Source hashes are recorded. See docs/text-input.md.

`input-password-reference.txt` executes unchanged native GTextInput.password,
Input.type, Text._typeset/UBBParser and TextInputAdapter.showInputElement under
minimal hosts. It records **128 text projections, 5 password toggles and 4 element selections**,
including astral/combining/ZWJ text, CRLF, literal HTML, UBB, translations and
readable empty-input prompts. The HTML host records the parser argument; no
HTML/browser layout is emulated. Core checks every projection and accessor
invariants. Source hashes are recorded in the header. Native DOM focus, keyboard,
IME, clipboard security rules and GPU rendering are outside this fixture.

`input-events-reference.txt` executes unchanged native Input focus/text accessors
and TextInputAdapter begin/end, validation/input and Enter methods, including
their promise continuations. Ten session traces cover editable/
readonly endings, changed/rejected/duplicate input, Enter/NumpadEnter, multiline
default insertion and prevented Enter. Bevy adapter regressions compare the
actual event sequences and final values. A separate native focused-assignment
trace records same-value setter notifications (a documented adapter difference).
The minimal PAL/DOM-value hosts supply element values/default newline insertion;
they do not emulate a browser, OS keyboard/IME or rendering. Source hashes are
recorded in the header. See docs/text-input.md.

`text-stroke-reference.txt` executes unchanged Laya 3.3 Text stroke accessors,
Text.renderText and TextRender.draw/drawOffscreen under minimal command/Canvas
hosts. It records
**7 width changes, 4 color changes, 6 native draw commands and 10 canvas traces**,
including fractional/nonpositive widths, unchanged setters, bitmap image-command
bypass, round joins and stroke-before-fill order at two target scales. Core checks
setter invalidation and command/raster policy invariants. Canvas hosts record calls,
not browser font pixels. TextRender.ts was retrieved from the same LayaAir_3.3
branch whose Text.ts matches the existing cached source; hashes are in the fixture.
See docs/text-stroke.md for Bevy raster/layout checks and compositing limits.

`list-selection-reference.txt` executes unchanged FairyGUI-layabox GList selection
methods with materialized GButton/GObject hosts. Its 184 steps cover all four
modes, Shift anchor retention, Ctrl toggles, replacement/add/remove/clear/all/
reverse, Common/non-button rows and selection-controller pages. Source hashes
are in the fixture. The host excludes virtual-list caches, browser input and
rendering. Core compares every selection/anchor/controller result; Bevy pointer
and window regressions validate adapter integration. See docs/list-selection.md.

`scroll-view-reference.txt` executes unchanged FairyGUI-layabox ScrollPane
visibility/position/refresh/tween methods, GObject rectangle/pivot methods and
LayaAir_3.3 Sprite transform/point methods under scalar/matrix-value hosts.
It contains
324 rectangle branches, 13 tween steps and 7 nested transforms. Source hashes
are in the fixture. The minimal hosts provide timers, matrices, content extents
and display-object values; they do not create windows, emulate a browser or
capture native GPU output. See docs/scroll-to-view.md for policy and limits.

`list-bar-reference.txt` executes unchanged FairyGUI-layabox ScrollPane setup/
dispose and GList numItems/getSelection under scalar display/list hosts.
Its 36 cases cover
three directions, four bar display modes and zero/three/seven rows, recording
bar attachment, selection and pane-owned disposal independently of list rows.
Source hashes are in the header. Core compares all cases and reruns the 184
selection traces with interspersed registered bars. No browser or native GPU
output is captured. See docs/list-scrollbars.md for adapter/window coverage.

`scroll-bar-input-reference.txt` executes unchanged GScrollBar constructExtension,
grip/arrow/background down/up handlers and ScrollPane directional/set-position
methods. The 216 cases cover both axes, page mode, start/middle/end positions, six hit
roles and before/at/after grip-origin branches. Scalar event/coordinate hosts
record logical positions after native refresh, propagation stops, native down bindings and
grip flags; they do not create a browser or OS window. Core compares logical
actions and stop boundaries; adapter/window checks test press/hold/release routing.
The recorded native grip flags do not alone prove adapter drag/fade parity.
Source hashes and host limits are in the fixture and docs/list-scrollbars.md.

`bar-visibility-reference.txt` executes unchanged native ScrollPane visibility/
completion/immediate-refresh methods and the actual GTween stack (TweenValue,
EaseType, EaseManager, GTweener, TweenManager, GTween). The 162 snapshots cover both
display modes, per-axis grip ownership, activity, idle refresh, unavailable bars,
delay, QuadOut alpha and completion. Core compares visible/alpha/grip values.
Scalar timer/display hosts supply runtime infrastructure without OS/window/GPU
output; source hashes are in the header. See docs/scrollbar-visibility.md.

`scroll-wheel-reference.txt` runs unchanged ScrollPane setup, scrollStep, wheel,
position and immediate-refresh methods plus its overlap block, Laya's DOM-wheel
branch and actual TouchInfo.bubble. It records 1,422 pane and 24
nested-bubbling cases; core compares logical/display positions and changed flags.
Scalar node/container/byte-buffer/DOM-value hosts replace platform infrastructure,
without a browser or OS/GPU output. InputManager.ts is cached from LayaAir_3.3;
the source URL, backend Pixel extension and validation limits are documented in
docs/scroll-wheel.md. Source method/block hashes are in the fixture header.

`scroll-pages-reference.txt` runs unchanged ScrollPane page getters/setters,
binding accessors, controller handling/feedback and position-request methods,
Controller selectedIndex/pageCount and GComponent.applyController. Its 324 getter/feedback and
96 controller-command cases compare page/selected/previous indices, logical
targets and animation requests. Scalar bounds/owner/timer/event/action hosts
replace engine infrastructure; no deferred refresh, browser, OS window or GPU is
created. Source method hashes are recorded. See docs/scroll-pages.md for adapter
initialization, reparenting behavior, actual-window checks and remaining scope.

`pagination-reference.txt` executes unchanged GList.updateBounds with scalar
item-size/position, alignment and bounds hosts. All 577 materialized pagination
cases compare positions, resulting sizes and content extents across inferred
and explicit row/column counts, auto resizing, variable fractional/source sizes,
empty/partial pages and the native Bag viewport. No virtual list, container
alignment, constrained sizing, OS or GPU is used. The native method hash is in
the header. See docs/scroll-pages.md for the remaining layout boundaries.

`scroll-alignment-reference.txt` executes unchanged native ScrollPane request,
refresh, alignment and type-1 tween methods plus GComponent/nonvirtual-GList
snapping and actual-size getters. All 2,970 command traces
compare refresh plus three ticks, and 120 direct snaps plus three clamped-axis
stop traces cover logical/display positions, accepted requests and tween state.
Eight stateful snapshots compare mid-tween displayed-direction alignment,
restarting a distinct request at the same endpoint and preserving an identical target.
Scalar node/bounds/event/timer hosts explicitly flush commands; no OS/GPU,
release inertia or virtual-list metadata is used. Source hashes are recorded.
The wheel, scrollbar-input and scroll-to-view hosts also execute alignment now;
scrollbar-down positions are recorded after native refresh. See
docs/scroll-alignment.md for precise API/Pixel/raw-position and timing limits.

`scroll-motion-reference.txt` executes unchanged native mouseDown/mouseMove/
mouseUp, target/duration and fixDuration, release alignment, type-2 tween and
GComponent snapping methods. Its 1,728 scenarios compare
33,408 logical/display/animation snapshots across directional/page/item flags,
bounce/inertia switches, fractional ranges, scaled coordinates, 30/60-frame and
desktop/mobile thresholds, held/reversed/long drags, deceleration, per-axis
return bounce and interruption. Scalar stage/node/bounds/timer/event hosts enter
accepted drags with zero sensitivity; no virtual loops, pull-lock callbacks,
browser, OS input or native GPU are used. Source hashes are recorded. See
docs/scroll-motion.md for adapter APIs, window checks and remaining scope.

`pull-refresh-reference.txt` executes unchanged native mouse/release methods,
lockHeader/lockFooter, checkRefreshBar, full handleSizeChanged and type-2 tween.
Its 648 scenarios
compare 12,960 position/size/visibility/lock/event/tween snapshots across axes,
header/footer presence, maximum sizes, fitting/equal/overflowing content, edges,
preassigned/callback locks, resizing and unlock. Scalar resource/display parenting,
stage/timer/bounds/event hosts replace Laya infrastructure; callback locks execute
inside native mouseUp. No browser, OS input, native GPU, virtual mapping or loop
mode is run. Source hashes are recorded, and the shared scalar host leaves the
existing scroll-motion fixture unchanged. See docs/pull-refresh.md.

`blend-reference.txt` executes unchanged LayaAir 3.3 BlendModeHandler and
WebRenderStruct2D in a scalar Node host, recording source SHA256 hashes,
216 three-level blend inheritance/isolation cases and 36 premultiplied blend
samples. Identity filters use native subStruct objects; basic Matrix/Vector/Rectangle
holders replace unused graphics infrastructure. No browser or native GPU is used.
See docs/compositing.md for the Bevy renderer and window verification.
