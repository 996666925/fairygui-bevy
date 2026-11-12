# Native nine-grid rendering

Exported Image and Loader image resources now draw native nine-grid stretch or
tiling through the same UI material used for fill masks. This replaces the
previous tiled-grid fallback warning and also connects grid/tile source metadata
to Loader visuals, including first-frame and changed-URL content from a foreign
package. The ordinary ImageNode stays as a source descriptor; its transparent
tint suppresses a duplicate draw. No additional grid-cell entities are created.

## Reference behavior

FairyGUI-Laya Image.doDraw builds Laya's sizeGrid as top/right/bottom/left plus
`tileGridIndice != 0 ? 1 : 0`. Draw9GridTextureCmd converts that final flag to
an all-parts mask. MeshFactory uses five bits, ordered top/bottom/left/right/center;
corners never tile. Therefore FairyGUI-Laya treats every nonzero tileGridIndice
as tiling all four edges and the center. The earlier documentation/helper that
treated this as nine independent cell bits did not match the reference path.

Grid borders keep their source dimensions while the target axis can fit them.
When width or height is smaller than that axis's two borders, the two borders
compress proportionally and its center collapses to zero. The other axis is
unaffected. Top/bottom repeat horizontally, left/right repeat vertically, and
the center repeats on both axes. The last partial tile crops its source fraction
and fills the remaining destination; it neither leaves a gap nor stretches a
whole tile into the tail.

The core `tile_grid::split` helper now uses the native five-bit mesh mask;
`tile_count` includes a final cropped tile and `tiles` returns destination/source
fractions. `tile_grid::axes` computes native target and UV axes. These are changes
to the earlier provisional helper semantics. Applications targeting FairyGUI-Laya
should normalize any nonzero package flag to all five tile parts, as the adapter
does. The internal ImageGrid component carries this normalized boolean.

## Atlas coordinates and material drawing

Source data includes original canvas dimensions, crop offset, grid and tile flag.
The material computes a virtual source UV rectangle by extending the actual atlas
crop back to its untrimmed canvas, then maps native grid axes into that rectangle.
Sampling outside the actual atlas crop is discarded. Zero borders at a subtexture
edge receive the native half-texel inset. Full standalone textures skip that inset.

Loader grid nodes occupy the full aligned content canvas. Plain image/movie
trim layout remains unchanged. Native tiled and stretched grids are handled
without a Bevy TextureSlicer corner-scale approximation: width/height compression
is computed independently. Existing image flips reflect the whole mapping;
fill polygons crop it on the same entity. Grid materials remain active even with
fillMethod=0 and retain their handle across resize, tint, alpha and mask changes.
URL replacement/disposal releases the previous visual and its material ownership.

Native invalid degenerate grids can produce nonfinite coordinates (for example,
a single border larger than the target with a zero opposite border). The adapter
does not upload those coordinates; an existing material becomes empty. This is
a bounded invalid-input behavior, not a claim to reproduce native NaN rendering.

## Evidence and validation

The checked-in native reference trace records results from unchanged type-stripped LayaAir
3.3 MeshFactory under a minimal Rectangle/VertexStream host. The fixture records
140 grid cases, target/UV axes and 688 emitted nonempty quads, including each
native tile bit, stretch, cropped tails, narrow dimensions, trimmed source UVs
and zero borders. It also executes unchanged bundled FairyGUI Image.doDraw for
eight tileGridIndice values to verify boolean normalization. Headers record both
source hashes. The fixture does not implement GPU sampling or native UI clipping.

The fixture records the unchanged `MeshFactory.ts` methods from LayaAir 3.3 and
the FairyGUI `Image.doDraw` method. Rust tests consume the checked-in fixture
without the original source checkout. The derived core geometry follows
LayaAir's MIT license notice at
`crates/fairygui-core/LICENSE-LayaAir`.

`tools/generate_nine_grid_package.py` derives NineGrid.fui from the official
Basics export. It changes package identity/URL IDs and sets 34 existing grid
image flags to nonzero; native component and atlas data stay intact. Tests and
smoke share the original Basics atlas files. Adapter tests verify grid materials
without fill, mask toggling, native target/UV/repeat parameters, a trimmed virtual
UV rectangle, foreign Loader atlas ownership, full canvas sizing and cleanup.

Windows smoke loads a direct exported Image, tiled foreign Loaders and a stretched
Basics Loader. It supplies a labeled-by-color texture with distinct corners,
striped edges and checkerboard center, then captures large/narrow/short targets,
partial tile tails, radial masks, flips, resize, inherited alpha and URL cleanup.
Captures are `target/fairygui-demo-nine-grid.png` and
`target/fairygui-demo-nine-grid-updated.png`.

These checks establish native branch/geometry/flag behavior and actual Bevy
rendering. Exact Laya filtering/rasterization, rotated atlas sprites, every
high-resolution/trim combination and per-cell arbitrary user flags beyond the
FairyGUI-Laya normalization are not established by these checks. General
renderer gaps remain tracked in README.
