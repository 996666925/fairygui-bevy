# Bitmap UBB commands, images and links

Bitmap fields with UBB or HTML enabled use `layout_bitmap_rich_text`. Plain bitmap
fields continue through `layout_bitmap_text`; ordinary rich text uses Bevy's
paragraph backend. Bevy 0.20.1 remains the latest stable version verified against
crates.io during this phase.

## Layout

The core entry point accepts explicit text, image, link-start and link-end
elements. Text elements retain their own requested size. It returns glyph
rectangles with element ownership, image positions, measured command bounds,
native link areas, logical text/object sizes and shrink scale.

Style commands share one line cursor. Wrapping can split a previous command or
move trailing commands to the next line to keep a word together across style
boundaries. The native 20-character suffix limit, punctuation retreat, UTF-16
surrogate boundary rules, per-advance rounding and two-pixel field padding are
retained. Mixed commands align vertically to the middle of the tallest command,
following Laya TextStyle's default alignItems. Every line reserves an additional
pixel for underline; leading is floored after scaling.

Images contribute their original canvas size and one horizontal pixel on each
side. Their size remains fixed during font shrink. Alignment, Both/Height,
Shrink and Ellipsis act on the whole command list. Ellipsis can replace an image
with a text command, using the preceding text style or the field's default style
and most recently measured font metrics. The native finalized width of a line
can survive later command moves; this behavior is represented in the reference.

## Bevy ownership and appearance

The adapter maps the shared native UBB/HTML parser's span indices to core elements. Bitmap
glyphs use the selected font's package; inline images use the text owner's
package/explicit foreign URL or configured Bevy asset source. Image trim offsets and crop sizes remain separate
from their logical layout canvases. Images in a bitmap paragraph are ordinary
children of its owner, so transforms, visibility, clipping, pooling and disposal
follow that owner without a Bevy paragraph or sibling-image positioning pass.
`InlineImageParts` continues to describe Bevy-paragraph sibling images.

Tintable glyphs use their explicit span color or default field foreground;
colored glyphs and inline images use white before sampled color effects. Links
now receive the native default underline. Bitmap
bold/italic tags do not synthesize glyphs, consistent with Text.renderText's
bitmap branch. Underlined text commands draw a separate rectangle matching the
native horizontal line's size/position. A TextDecoration marker restricts
background-color synchronization to these rectangles; normal image/glyph node
backgrounds remain transparent. HTML strike commands also draw their native
extended rectangle. Gray/filter/opacity use the existing appearance
and sampled-image material paths, with the composed-text limits described in
[color filters](color-filters.md).

BitmapRichText snapshots translated content, text properties, logical size and
image-source identity. Content/size/style changes rebuild its tracked parts;
late foreign package or external Image assets and asset removal also trigger reflow.
External images preserve independent explicit axes and fill natural axes on load;
pending/failed views remain hidden. See [external inline images](external-inline-images.md).
Gray alone
updates appearance without changing glyph/part identity. Unrelated application
children and the owner entity survive rebuilds and renderer changes. Empty UBB
content retains bitmap ownership with zero measurements.

Link hit areas come from native line/command rectangles, including wrapped
continuations. Input transforms normalized physical node coordinates back into
logical pixels before testing them. Each opening tag keeps its own identity,
even when adjacent tags share an href. A generation token and current content/
properties/size validation reject stale captures. Successful clicks use the same
GuiLink message and core Link bubbling path as ordinary rich text; hrefs are
delivered verbatim.

## Evidence

Run . It executes unchanged,
type-stripped LayaAir 3.3 BitmapFont, Text.doLayout, Text.renderText and their
helpers. Explicit styled elements feed a minimal image/link positioning host;
drawImage records glyph rectangles. Native UBB/HTML parsing, image loading and GPU
rasterization are not emulated by this host. Source SHA256 hashes are in the
fixture header.

`bitmap-rich-reference.txt` records **955 layouts, 6,208 drawn glyphs, 406 image
positions and 287 link areas**. Cases include fixed/scalable fonts, mixed sizes,
cross-command words/punctuation, images, adjacent links, all auto-size modes,
surrogate/missing glyphs, image-only paragraphs and empty links, with seeded
mixed-command cases. Core tests compare metrics, commands, glyphs, images and
link rectangles. Adapter tests instantiate the actual exported font/image assets
and exercise retained ownership, tint, alpha, underlines, pooling, rebuilds,
foreign image arrival/removal, DPI conversion and real input message routing.

Windows smoke captures six actual package samples covering colored/tinted fonts,
fixed/auto-size/shrink/ellipsis modes, then gray and content/font/width updates.
Captures are `target/fairygui-demo-bitmap-rich.png`, `-gray.png` and `-updated.png`.
These are geometry/state checks and Bevy render captures, not a comparison to
native framebuffer pixels.

## Remaining scope

The shared parser now follows the [native UBB/HTML pipeline](markup.md), including
link-default underline, entities, percentages and the shared style stack. Form
objects, inline atlas animation, bitmap input caret/selection, native antialiasing of decoration
strokes, all rotated/HD glyph cases and native framebuffer parity remain open.
Plain bitmap fields use their own native per-line underline bounds. Both paths
share stroke-bound arithmetic; field underline edits preserve the logical owner.
Image-command decorations remain unimplemented because native pooled command
fontSize state is not established by the text-decoration fixture.

Native shrink can fail to converge for fixed fonts or fixed-size images that
cannot fit. The core bounds binary search to 128 iterations. Extremely small
boxes can produce native negative glyph extents; core traces retain those
measurements but Bevy omits negative render quads. Image-only ellipsis can retain
native NaN command width when no text font was measured; max-line measurement
ignores it while default-style glyph geometry remains drawable. These cases do
not establish complete malformed-input or render equivalence.
