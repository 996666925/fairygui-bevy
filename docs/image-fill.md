# Image fill masks and progress bars

Image and Loader image/movie content support horizontal, vertical and radial
90/180/360 fill masks. The adapter crops the texture in place, retaining the
object's full layout/pivot/input rectangle. The earlier horizontal/vertical
path shrank the node and compressed its entire texture; radial fills previously
warned and drew the full image. All five modes now use native FillUtils polygons
through Bevy 0.20.1's UI material pipeline.

```rust,ignore
use fairygui_core::widget::FillMethod;

runtime.store_mut().set_image_fill(
    image,
    FillMethod::Radial360 as u8,
    0,     // Top; radial90 uses TopLeft at the same code.
    true,  // Clockwise.
    0.25,
);
runtime.store_mut().set_image_fill_amount(image, 0.75);
// Disable masking and return to the ordinary ImageNode draw path.
runtime.store_mut().set_image_fill(image, 0, 0, false, 1.0);
```

Both setters accept f32/f64 amounts and reject nonfinite API input. Finite raw
amounts are retained: values <=0 draw nothing and values >=0.9999 draw the full
rectangle, matching native FillUtils. Fill amounts are stored as f64 so runtime
JS Number thresholds remain distinct from serialized Float32 values. Package
decoding promotes the original Float32 value without changing it.

Radial90 origin codes 0/1/2/3 mean top-left/top-right/bottom-left/bottom-right.
Radial180/360 use top/bottom/left/right. Horizontal/vertical native aliases 0
and 2 start at the left/top; 1 and 3 start at the right/bottom. Non-square sizes,
quarter/half boundaries, clockwise reversal and the native near-complete cutoff
use the unchanged branch structure from display/FillUtils.ts.

## Rendering and ownership

A filled image keeps its ImageNode as the atlas/frame/slicing descriptor and
receives one MaterialNode on that same entity. Its ordinary image tint becomes
transparent to suppress the duplicate unmasked draw. The material samples the
same texture and atlas rectangle, with the same nine-slice/tile parameters.
Image flips reflect texture mapping and the mask together. Current effective
ancestor alpha, color and existing grayed/filter approximations feed the material.

For loaders, the material attaches to the existing ContentVisual. Its mask is
defined on the full aligned/scaled content canvas before sprite trimming; the
trimmed node maps back into that domain. Movie frame changes update the texture,
atlas rectangle and original-frame canvas. Loader components and error signs
are outside this image-content mask, matching their separate native attachments.

Materials retain their handles across progress and texture changes. Disabling
fill removes the material and restores the ordinary image tint when the image
does not also use native nine-grid rendering. Nine-grid content retains the same
material with a full mask; see [nine-grid.md](nine-grid.md). Normal entity
disposal and strong-handle release reclaim the material. Generic appearance
synchronization excludes filled material nodes; the fill system supplies their
current tint, avoiding a visible duplicate image or perpetual color rewriting.
Input-only hosts initialize material assets without requiring RenderApp.

The shader uses four derivative-based mask coverage samples and Bevy UI's
normal transform, clipping and ordering pipeline. UV mapping follows Bevy
0.20.1's texture-slice pipeline; its MIT notice is retained at
`crates/fairygui-bevy/LICENSE-Bevy`. Exported nine-grid images now use native
grid-axis/tile mapping on that same material; exact native Laya antialiasing
remains outside the validation claim.

## Progress behavior

GProgressBar now parses its resource extension's title type/reverse flag,
captures horizontal `bar` and vertical `bar_v` dimensions/offsets, and initializes
its serialized value after instance setup. Native default value/max are 50/100.
An image or loader with an active fill method receives percent (or 1-percent
when reversed), retaining its size. Other bars resize along the axis specified
by their name, retaining the initial parent-to-bar dimension delta on owner
resize. Reverse bars shift from their captured start position. Optional `ani`
receives floor(percent*100), and percent titles use floor as native does.

Percentage math retains double precision through title/frame calculations. This
matters at ordinary decimal fractions: e.g. native floor((29/100)*100) can be 28
because of JS Number rounding. The executable progress fixture covers this.
The existing public set_progress contract still clamps integer values to min/max;
this phase does not add the native floating value/tweenValue API.

## Evidence and validation

- The checked-in native reference trace records results from unchanged bundled FillUtils and
  records 1,680 masks: all five methods, four origins, both directions, square
  and non-square dimensions, empty/full values, quarter/half boundaries and
  Float32 versus JS Number near-complete thresholds.
- The checked-in native reference trace records results from unchanged GProgressBar.update
  and setFillAmount with minimal Image/Loader/size/text hosts: 24 cases and
  144 updates cover both axes, fill/non-fill content, reverse, owner resize,
  titles and ani frames. Both fixtures record source hashes and are consumed
  by Rust tests without Node or reference sources.
- Core tests also load the official Cooldown package and check progress changes
  leave image geometry intact. Adapter tests check atlas/material retention,
  trim-domain mapping, texture replacement, opacity, zero/disabled fill,
  slicing/tiling/flipping parameters and resource cleanup.
- Windows smoke holds Cooldown at two progress states, captures both, and adds
  an 18-loader table showing all five fill modes at quarter/half/three-quarter
  progress. Captures are `target/fairygui-demo-radial-cooldown.png`,
  `target/fairygui-demo-radial-cooldown-updated.png` and
  `target/fairygui-demo-fill-masks.png`.

This establishes native mask coordinates and bounded progress control behavior,
plus actual Bevy drawing. It does not establish native pixel parity, every
texture-trimming/slicing combination or exact Laya scheduling. Textures continue
to use the adapter's existing source/atlas rules; general image-renderer gaps
remain tracked in README.
