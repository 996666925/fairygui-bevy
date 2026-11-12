# Color filters and local grayscale

The rendering path described below was the original local-filter implementation.
With a RenderApp, filters now isolate the complete object subtree and apply the
matrix after composition, including containers, text overlaps and Loader
component content. See [blend modes and isolated effects](compositing.md) for
current ownership, alpha transfer, validation and remaining scope. Renderer-free
apps retain the local projection described here.

Bevy 0.20.1 remains the latest stable release verified against crates.io during
this phase. Local exported Image, standalone MovieClip, Loader image/movie,
bitmap glyphs and rich-text inline images now execute a color matrix after
sampling their current atlas texture.
They share the existing FillMaterial with fill masks and native nine-grid UVs.

## Parameters and reference behavior

`ColorFilterParams` contains the four raw FairyGUI package/transition values in
brightness, contrast, saturation, hue order. Each defaults to zero and clamps to
[-1, 1]. NaN follows native `value || 0`; infinities clamp to the corresponding
endpoint. This corrects the old factor/degree interpretation of package values.

FairyGUI-Laya ColorMatrix adjusts hue, contrast, brightness, then saturation,
left-multiplying each adjustment. Hue is a half-turn proportion (`0.5` is 90
degrees); contrast uses scale `1 + value` and offset `128 * (1 - scale)`;
brightness adds `value * 255`; saturation uses scale `1 + value` and weights
0.299/0.587/0.114. Construction retains double-precision intermediates, then
normalizes matrix offsets by 255 for normalized color channels and uploads f32.

The independent `ColorMatrix::brightness`, `contrast`, `saturation`, `hue` and
`from_params` convenience functions retain their documented factor/degree
semantics. They are not aliases for FairyGUI package parameters. The public
`ColorFilterParams::tint` remains a fallback applying the native matrix to white;
it cannot represent channel mixing or additive offsets on textured content.

Grayscale follows the **legacy ToolSet ColorFilter path**: local `grayed` uses
the Laya gray matrix with equal RGB rows 0.3086/0.6094/0.082, temporarily overriding
the stored color filter. Changing a filter while gray preserves it; ungraying
restores the newest filter. Saturation -1 uses different native weights and is
deliberately distinct. The newer PostProcess2D path maintains separate color and
grayscale effects; this implementation does not claim its effect-stack behavior.

## Rendering and ownership

The shader converts Bevy's linear texture sample and base tint to encoded sRGB
channels, multiplies those base colors, applies/clamps the native matrix, and
converts the result back to linear for Bevy UI blending. The inspected Laya 3.3
ColorEffect2D fragment shader applies its matrix to encoded premultiplied samples
and adds offsets divided by 255, weighted by alpha. For FairyGUI's four
alpha-preserving adjustments, straight RGB transformation followed by alpha
blending preserves transparent texels. Bevy's final blend occurs in linear
space; this is not a claim of pixel equality with every Laya framebuffer mode.
Images without an active effect retain the existing ordinary Bevy tint path.
The inspected shader is LayaAir 3.3's
[ColorEffect2D.fs](https://github.com/layabox/LayaAir/blob/LayaAir_3.3/src/layaAir/laya/display/effect2d/shader/ColorEffect2D.fs).

Base tint and effective ancestor/object opacity are read independently from the
model each frame, so filters and gray are not applied twice or accumulated.
Material handles survive parameter changes, gray toggles, transitions, source
texture/frame changes, masking and grid updates. With neither effect, mask nor
grid, the material is removed and ordinary ImageNode rendering resumes. ImageNode
remains the atlas/frame descriptor and retains the existing layout/input owner.

Pure graph fill/stroke colors and ordinary text foreground colors apply the same
local matrix on CPU. Grayed text's default foreground uses #AAAAAA while retaining
its stored color. The old public GRAYED_TINT constant is retained for source
compatibility and is no longer used by the runtime.

Bitmap glyphs and inline images resolve their text owner through TextTintSource.
Tintable glyphs use the raw text foreground; untinted colored glyphs and inline
images use white before the sampled matrix. Grayed default text foreground first
becomes #AAAAAA. Explicit span colors keep their override. The matrix is applied
once, after texture/tint multiplication, with effective ancestor/object alpha.
Each sampled quad uses its own full rectangle, so a tiny owner cannot mask a
larger glyph. Effect changes preserve glyph entities and atlas crop descriptors;
text rebuilds and linked sibling disposal reclaim their material assets.

## Evidence

The checked-in fixture records results from the unchanged native ColorMatrix and
ToolSet.setColorFilter code, using a minimal ColorFilter host to record the
selected matrix. Source SHA256 hashes, 48 matrices, 336 RGBA samples and five
legacy gray/filter states are recorded; no native GPU is simulated.

Core tests compare every matrix coefficient and color sample; adapter tests
cover zero defaults, raw tint/effective alpha, repeated sync, gray precedence,
filter restoration, interpolated transition parameters, movie/Loader visual
ownership, texture replacement, grid/mask/flip stability, disable and asset cleanup.

Windows demo smoke captures six synthetic swatch panels and actual package
Loader image/movie sources. It then changes hue on the existing objects and
captures again after multiple rendered frames. Verify the 96 physical swatch
sample positions against the CPU matrices and Bevy's linear alpha blend with:

```powershell
cargo run -p fairygui-demo -- --smoke-test
pwsh -NoProfile -File tools/check_color_filter_capture.ps1
```

Captures and position/color CSV files are under `target/`. The screenshot check
uses Windows System.Drawing, requires no Python imaging dependency, and allows
two byte levels of channel error. It checks actual Bevy shader output against
native-verified matrix arithmetic, rather than comparing screenshots from Laya.
Initial panels in row order are original, saturation -1, hue +0.5, combined,
grayed over combined, and brightness +0.25 with alpha 0.5. The updated capture
uses hue -0.5 on every panel, with the last retaining alpha 0.5. This run passed
all 96 samples with maximum error 1/255; 699 workspace tests, strict all-target
Clippy, formatting, and the Windows smoke of 15 demos twice/20 Basics pages
also passed.

### Sampled text images

 records native draw-tint
selection before the legacy matrix: 4 source policies, 28 states and 84 RGBA
samples. It executes Text.renderText and FairyGUI's GTextField/GObject color/gray
handlers with a minimal draw/ColorFilter recorder. The white-image policy is a
representative explicit white source, not execution of HtmlImage. See the adapter
fixtures README for the precise host boundary.

Adapter tests also instantiate BitmapText.fui's native image-backed and atlas
fonts, and Basics.fui's actual sibling inline images. They check crop/layout
retention, foreground/white selection, alpha, material reuse, gray/filter changes,
rebuild, disable and disposal. Headless tests give the real atlas handles
placeholder texture storage; Windows smoke loads the actual PNG pixels.

The Text page smoke draws a 3-by-3 grid: colored bitmap, tinted atlas and inline
image sources, with near-identity brightness +0.0001, hue/saturation, and gray
overriding a stored filter. The gray column has effective alpha 0.6. One actual
glyph/image per panel receives the synthetic RGBA texture; remaining exported
glyphs/images remain visible. A second capture changes every panel's combined
filter and clears gray, retaining the same entities. Check both captures with:

```powershell
pwsh -NoProfile -File tools/check_color_filter_capture.ps1 `
  -CapturePrefix fairygui-demo-text-image-colors `
  -SamplesPrefix text-image-color-samples
```

All **144 text-image GPU samples** passed with maximum channel error **1/255**,
including transparency, nonwhite text tint and default gray foreground. Existing
96 image samples still pass at 1/255. Workspace validation is now **724 tests**,
strict all-target Clippy, formatting and demo build. This verifies local sampled
colors against native arithmetic, with Bevy linear blending against white.

## Original local-path limits and current scope

- Container filters and Loader component/error-sign content now participate in
  the shared offscreen compositor when a renderer is available.
- Component recursive assignment, the `grayed` controller, button disabled-page
  precedence and serialized instance initialization now follow native handlers;
  see [component grayscale state](grayed-state.md). These are assignments and
  controller pages rather than inherited container filters.
- Text filters now apply after glyph/stroke/inline-image composition in the GPU
  path. Renderer-free local projection still cannot represent those overlaps.
- Native text rasterization, text outlines, rotated atlas combinations, custom
  alpha-changing matrices and PostProcess2D effect ordering are not established.
