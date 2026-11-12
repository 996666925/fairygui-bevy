# Rich-text images and lifecycle

Image layout anchors are Bevy 0.20 `InlineBox` children of the text node. They
participate directly in Parley shaping, line breaking and measurement, including
image-only paragraphs. Image quads remain siblings of the text node so `ImageNode`
can select a rectangle inside a FairyGUI atlas; Bevy's built-in `InlineImage`
widget accepts whole textures, without an atlas rectangle. This physical parent
is independent of ownership.

`InlineImageOwner(text_entity)` is a Bevy 0.20 relationship, exported from
`fairygui_bevy::runtime`. Its automatically maintained inverse `InlineImageParts`
exposes `iter()` for inspection. The relationship uses linked_spawn: despawning
the text entity reclaims all its owned images even when they are layout siblings.
Application code should use the core model APIs for object lifecycle and let the
runtime maintain these view relationships.

During synchronization, images follow their owner's current logical parent and
sibling order. A standalone text root has standalone image roots. Explicit text
hiding, group display changes and stage removal also hide the image. Object pools
retain both owner and images for reuse. Before queued root/window/subtree removal,
the runtime reparents images owned by surviving text objects, protecting a child
detached earlier in the same application frame from recursive Bevy despawn.

The image's local quad follows the text's rotation and scale. Because siblings
transform about different centers, the adapter compensates the center offset using
the measured text size. This also handles mirroring and nonuniform scale. Each
box's geometry is converted from physical to logical pixels. Images preserve their
texture colors and inherit effective opacity/filter rather than text foreground.
The box reserves the original sprite canvas plus Laya's one horizontal pixel on
each side; the visible atlas quad uses the exported trim size and offset. Trimming
therefore cannot stretch the artwork or change the paragraph's image advance.

Image positioning runs in PostUpdate after Bevy's text system and before UI
clipping. The adapter updates its atlas quad's computed size and global transform
from the current text box, including the text content inset and original trim
offset. It follows the same transform composition as Bevy's inline-image extractor.
New/reflowed images can render in the same frame as their measured paragraph;
missing anchors clear their computed size and hide them.

Changing rich text rebuilds only its tracked text spans, inline boxes and owned
images, leaving model children intact. UBB fields keep this span representation
even when initially plain, so an edit can introduce new styles/images. Rendering
snapshots include translated source, field style and GUI font. Unchanged snapshots
retain their entities. New images remain hidden until the rebuilt paragraph
contains a matching inline-box entity; missing layout also hides images.

Static external images also require a loaded Image asset before becoming visible.
Snapshots retain request handles and natural dimensions; arrival, removal or size
changes rebuild only tracked content. Explicit HTML axes reserve space while
pending or failed. Replaced/disposed requests cannot recreate old views. External
paths resolve from the configured Bevy asset root, while package URLs retain the
package rules below. See [external inline images](external-inline-images.md).

Rebuilds resolve resources using the original package and asset directory. Moving
text into another root, including one with the same package ID in a different
directory, does not change its source. Pending root generation reuses an existing
mapped text view instead of silently replacing it and leaking owned parts.

Tests use the official Basics/Demo_Text rich-text resource. They cover dynamic
content, missing image addresses, opacity/order, reparent/pool/reentry, standalone
roots, owner destruction, same-frame root disposal, cross-directory resource
ownership and transformed quad corners. Window smoke also verifies actual Bevy
run layout and captures transformed and rebuilt image states.

This is bounded lifecycle and rendering-state verification, not native pixel
parity. Image sizes and advances no longer depend on a placeholder glyph/font.
Paragraph baselines and line metrics still come from Parley; Laya's default
`alignItems=middle`, per-line rounding and extra underline pixel can differ.
The layout tests exercise the actual Bevy TextReader/Parley pipeline with the
tracked demo font, in addition to real-window checks.

The image margin and trim rules follow LayaAir 3.3 `Text.ts` (`addCmd`) and FairyGUI
`UIPackage.ts` (`getItemAsset`): nonempty HTML objects receive one horizontal pixel
on each side, while Texture creation retains the sprite's original canvas and
trim offset. Local source references are under `.refs/LayaAir-3.3` and
`.refs/FairyGUI-layabox`.

## Loaded packages and links

Both `ui://PackageName/ItemName` and `ui://<package-id><item-id>` image URLs
resolve across loaded `Assets<FuiAsset>`. Bare names remain local. A foreign URL
never falls back to a coincidentally matching local name. The original owner
package/directory wins for local URLs even after reparenting. If multiple copies
of a foreign package are loaded, the smallest `res_key` in lexical order wins,
so iteration order cannot randomly change the texture directory. The host must
load/retain package assets; rich text does not fetch dependency packages itself.
Loading/unloading a referenced package changes the rendering snapshot and rebuilds
its text-owned parts. Unresolved, unchanged sources do not rebuild every frame.

UBB URL scopes preserve an opening-tag identity through nested styles and images.
Separate adjacent tags with identical hrefs remain distinct. Link hit testing uses
each actual Bevy text-run or inline-box rectangle in physical content coordinates (including padding)
and the owner's normalized cursor, so wrapped lines do not create a giant union
rectangle covering empty gaps. Bevy's inverse UI transform supplies this cursor
for rotated/scaled/mirrored nodes. The normal pointer target policy still applies:
stage, visibility, touchability, modal ownership, occlusion, capture and drag
thresholds must all pass. Press/release must stay within the same link identity and
rich-text generation. Changes to content/style/font reject stale geometry.

`EventKind::Link` carries `EventData::Text(href)` and bubbles from the text object.
`GuiLink { href, event }` publishes once in `GuiSystems::Publish`, before application
handlers in `GuiSystems::Respond`. Draining core events does not consume this
message. Hrefs are preserved verbatim, including `event:`. The runtime never opens
external URLs automatically. The Basics demo consumes this API to implement the
official link response and clean up its old image in the same frame. The official
replacement references MovieClip `t9fj5f` inside `[img]`; `GHtmlImage` accepts only
textures, so that non-image resource is skipped, matching its `instanceof Texture`
check.

The event semantics follow LayaAir 3.3
[`HtmlLink.ts`](https://github.com/layabox/LayaAir/blob/LayaAir_3.3/src/layaAir/laya/html/HtmlLink.ts):
its click handler calls `owner.bubbleEvent(Event.LINK, element.getAttrString("href"))`.
The FairyGUI reference is `.refs/FairyGUI-layabox/demo/src/demo/BasicsDemo.ts`,
`playText` / `__clickLink`; its sample href is `event:xx`.
