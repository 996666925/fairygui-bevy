# Plain bitmap text layout

The runtime now measures and draws plain bitmap text using FairyGUI-Laya and
LayaAir 3.3 behavior. Bare font names resolve locally; canonical and named `ui://`
fonts can resolve in another loaded package and retain that font's own atlas.
The runtime remains on Bevy 0.20.1.

## Measurement and drawing

`core::bitmap_text::layout_bitmap_text` takes a BitmapFontData, TextFieldData,
translated text and logical object size. It returns glyph rectangles, measured
text size, resulting Both/Height object size and the selected shrink scale.

- autoScaleSize enables requested-font-size/design-size scaling. A fixed-size
  font retains its design-size glyphs during ordinary layout.
- Each advance is rounded independently, following JS Math.round. Glyph offsets
  and visual sizes remain fractional after scaling. Summing float advances then
  rounding once would move later glyphs to different positions.
- Missing UTF-16 glyphs have zero width and produce no draw. The package loader
  does not synthesize a space glyph, unlike Laya's external XML-font loader.
- Native FairyGUI text padding is two pixels on each side. Nonempty line height
  uses the bitmap line height plus one pixel reserved for underline, and leading
  is floored after shrink scaling. Blank explicit lines retain line height.
- Plain-text wrapping follows the native ASCII word suffix/20-character limit,
  punctuation retreat and forced one-character behavior when a box is too narrow.
- Center alignment floors the horizontal remainder; right alignment keeps its
  full remainder. Vertical middle floors its offset; bottom retains it. Only
  positive remainders shift the text.
- Both/Height auto-size return logical measurements including padding. Shrink
  uses native repeated measurement/binary search; Ellipsis keeps the selected
  lines and applies the native final-command truncation rule. A missing ellipsis
  glyph remains missing rather than using the default system font.

BitmapFontData::measure_width and measure_width_at now follow native per-step
rounding and missing-glyph rules. This changes the old helper's behavior that
assigned every missing character x_advance. The public Bevy text::layout_line
and layout_text helpers retain their earlier simple layout behavior for callers;
the runtime now uses the core bitmap layout instead.

Image-backed font glyph dimensions come from the logical branch image before
HD texture resolution, matching UIPackage.loadFont. The Bevy glyph node maps
the texture's untrimmed virtual canvas into its actual crop: scaled sprite
offset moves the node and scaled crop dimensions size it. This prevents a trimmed
image from stretching across the entire glyph cell. Atlas-backed font glyphs
keep their font-atlas rectangle. Rotated/HD combinations remain bounded scope.

## Synchronization and ownership

BitmapGlyphs is retained even when the string is empty or every glyph is missing.
The owner does not acquire a fallback Bevy Text node. BitmapTextMetrics records
measured text/object sizes in logical pixels. Both/Height consume these metrics
instead of Bevy's physical node size, including zero sizes. Shrink/Ellipsis keep
their specified outer dimensions. Auto-size may trigger one final SIZE rebuild,
then stabilizes; it does not feed DPI-scaled sizes back into the model.

TEXT or SIZE changes rebuild only glyph entities, preserving the GObject owner
entity. This includes set_object_font_size, controller FontSize application,
changed wrapping width and translated text on subsequent text changes. Owner
disposal releases the current glyph tree through ordinary Bevy hierarchy cleanup.
Initial creation and rebuild read the same owning font/package and translation
path. Color/opacity continue through the existing appearance system.

`ObjectStore::set_object_font(id, Option<&str>)` changes the requested font;
`set_object_ubb_enabled(id, bool)` toggles ordinary-text UBB parsing. Both also
forward through resolved button/label/combo title text fields, coalesce TEXT
dirty flags and leave saved gears intact. Font selection is reconciled after
dirty synchronization, independently of the dirty queue, so an explicit foreign
font can become available when its asset arrives later. Obsolete requests cannot
replace a newer selection. Removing the source asset returns ordinary rendering.

Creation and selection changes share runtime::text_fonts::spawn_text. A renderer
switch reclaims only its own glyphs, spans and linked inline images; logical and
application children and the owning entity survive. Switching to bitmap also
removes Bevy's old paragraph, measured geometry and ContentSize, and clears text
alignment padding. Restoring ordinary text creates a fresh paragraph; restoring
UBB recreates its spans/inline images under the same owner. GuiFont changes on an
ordinary input preserve its EditableText component rather than resetting editing
state. A bitmap input still lacks native caret/selection rendering while bitmap
content is active; returning to ordinary text restores editability.

Canonical native font selection follows GTextField.font preloading the URL and
then Laya Text.font selecting a registered bitmap or ordinary fallback. The
adapter's named foreign URLs and polling of loaded FuiAsset storage are resolver
conveniences; they do not claim Laya's global registration/async loader lifetime.
Ordinary font names now use Bevy family lists with GuiFont/default fallback;
external `.ttf`/`.otf` assets load asynchronously, with selected sources shared by
ordinary paragraphs, rich spans and inputs. See [ordinary fonts](raster-fonts.md).
Font branch/HD context changes
and in-place package hot reload are not established by selection polling.

## Native reference and fixtures

The ignored source files used by the generator are:

- LayaAir 3.3 `src/layaAir/laya/display/BitmapFont.ts`
- LayaAir 3.3 `src/layaAir/laya/display/Text.ts`
- FairyGUI-Laya GTextField padding/auto-size and UIPackage.loadFont, inspected
  for package/adapter behavior outside Text's own methods.

Run . It executes unchanged
type-stripped BitmapFont, Text.doLayout, Text.renderText and their helper functions
with synthetic glyph metadata and a minimal display/draw recorder. Source hashes
are recorded in bitmap-text-reference.txt. The native post-layout callback host
models GTextField's Both/Height size assignment. It records **422 layouts and
2,391 drawn glyph rectangles**, covering fixed/scalable fonts, tint flags, sizes,
missing glyphs, blank/explicit lines, UTF-16 surrogate text, words/punctuation,
alignment, all five auto-size modes, narrow boxes and seeded mixed text cases.
These checks exercise native layout/draw arguments, not native GPU rasterization.

`python tools/generate_bitmap_text_package.py` derives BitmapText.fui from the
official Basics export. Changes are package identity/URL/dependency data, the
two native fonts' auto-scale headers and selected Demo_Text instance properties
(content, positions, visibility and layout settings). Font glyph records and
original atlas pixels are retained. Nine fields exercise native image-backed
HitNumber and atlas-backed BMFontTest in several sizes/layout modes.

Adapter tests check actual font-atlas glyph scaling and logical auto-size at
1.75 DPI, stability for 60 frames, empty/missing ownership and recovery,
translation on rebuild, resize wrapping, image-glyph crop dimensions and atlas
ownership. Windows smoke renders both font families, changes fontsize/width,
checks measured bounds/retained owners, clears/restores text and verifies disposal.
Captures are target/fairygui-demo-bitmap-text.png and
target/fairygui-demo-bitmap-text-updated.png.

This phase passed 721 workspace tests, strict all-target Clippy, formatting and
the Windows smoke of 15 demos twice/20 Basics pages. Both bitmap captures were
inspected for native image/atlas glyph sizing, alignment, truncation and dynamic
font/width updates. Existing color-filter captures passed all 96 GPU samples,
maximum channel error 1/255. These are
native geometry traces and actual Bevy rendering checks, not native framebuffer
pixel equality.

## Remaining scope

This layout entry point is for plain text. UBB bitmap fields now use the separate
[multi-command layout](bitmap-rich-text.md), with sizes/colors, inline images,
links and underline/strike rectangles. The [native markup parser](markup.md)
also handles raw HTML. Plain fields now draw native underline bounds from their
measured lines, including wrap, alignment, shrink and ellipsis. The core
`BitmapTextLayout::underlines` retains logical stroke rectangles; Bevy keeps these
in `BitmapDecorations` separately from sampled `BitmapGlyphs`, so untinted fonts
still use the field foreground for their underline. `set_object_underline`
supports runtime edits and compound title forwarding; plain Bevy paragraphs also
update their Underline component. Color/alpha/gray changes retain decoration
entities, while reflow, renderer switching and disposal reclaim owned parts.
Bitmap input selection/caret rendering remains unfinished. Sampled-channel
gray/color filters now cover both bitmap font families, preserving native tint
selection; see color-filters.md for reference and GPU checks. Filtering composed
overlapping text still needs offscreen rendering.

 executes the same
unchanged native layout/render methods with underline and strike enabled. The
separate deterministic fixture records **422 layouts, 2,391 glyphs, 757 underlines
and 757 strikes**. Core compares actual per-line underline rectangles and shared
strike-bound arithmetic. This establishes horizontal stroke geometry, including
the half-thickness offset around native drawLine's center, without establishing
native stroke antialiasing. Native FairyGUI exposes a field underline flag;
strikethrough remains an HTML style, rather than a new binary field flag.

Native shrink can loop without progress for a fixed-size font that cannot fit
its fixed line height. This implementation bounds that degenerate search to 128
iterations; such inputs are not claimed to match an indefinitely running native
layout. Arbitrary malformed sizes, zero/negative design-font metadata, complete
Unicode shaping and all rotated/HD source combinations are not established by
the fixture. The renderer's normal texture sampling and native framebuffer pixel
parity remain outside these geometry checks.
