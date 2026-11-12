# Polygon rendering

GGraph polygon and regular-polygon content uses Bevy 0.20.1's `UiMaterial`
pipeline. Rectangles and the existing ellipse approximation continue to use
ordinary UI backgrounds/borders. Package polygon controls now display instead
of warning and remaining blank.

Use `ObjectStore::set_graph(id, GraphData)` to replace a graph's drawing command
without replacing its object or Bevy view. Controllers and transitions that
change graph color continue to use the ordinary color property path.

```rust,ignore
use fairygui_core::{color::Color, widget::GraphData};
runtime.store_mut().set_graph(graph, GraphData {
    graph_type: 4,
    sides: 5,
    start_angle: -90.0,
    line_size: 3,
    line_color: Color::new(0.7, 0.1, 0.05, 1.0),
    fill_color: Color::new(1.0, 0.65, 0.05, 1.0),
    ..Default::default()
});
```

## Geometry and drawing

Arbitrary polygons retain their explicit local coordinates when the object's
dimensions change. Regular polygons regenerate their points using the smaller
object dimension for both radius and center, matching native GGraph even when
the object is not square. Per-vertex distance coefficients default to 1 for
missing/NaN entries. Start angles use degrees in screen coordinates. Float64
intermediates preserve native JS Number precision through the stroke joins;
the GPU receives Float32 vertices.

The closed stroke follows LayaAir 3.3 BasePoly.createLine2, including close-point
filtering, center alignment, miter intersection, bevel fallback and reversed or
near-parallel segment handling. DrawPolyCmd shifts odd positive line widths by
half a pixel. The surface bounds include outlying vertices and stroke miters,
so the graph object's layout rectangle does not crop its own geometry.

Each polygon owns one material surface inside its existing model view, plus a
material and storage-buffer asset. The surface retains the same entity and
buffer handles across geometry/color changes. Bevy handles UI clipping,
transforms, hierarchy visibility and draw ordering; the material receives the
core model's effective ancestor alpha and existing tint/grayed approximation.
Generic background-color synchronization excludes these surfaces, keeping
transparent areas outside the contour clear. Shape switching removes the old
surface. Normal view disposal and strong-handle release reclaim its assets.

The fragment shader computes even-odd fill coverage and union coverage of the
native stroke triangles. Four derivative-based samples provide edge coverage
under rotation and scaling. It composites fill/stroke once per sample, avoiding
triangle seams. This costs work proportional to polygon complexity per covered
pixel; it is not a general high-volume vector batching implementation.

## Input

Native GGraph attaches its graphics to Laya HitArea. Polygon hits use the actual
unshifted contour, independent of fill/stroke color, rather than the full layout
rectangle. Bevy surfaces preserve the graph's GObject ID for event routing and
convert their enlarged surface coordinates back into object space. A concave
cutout can therefore pass input to a lower widget. Empty graphs do not hit.
Existing rectangular/ellipse hit behavior is unchanged in this phase.

## Evidence and limits

The checked-in native reference trace records results from the unchanged bundled GGraph and
type-stripped LayaAir 3.3 BasePoly under minimal graphics/Vector2 hosts.
`crates/fairygui-core/tests/fixtures/polygon-reference.txt` records source hashes,
21 cases of native points/stroke vertices/indices and 756 native contour hit
samples. The BasePoly source is from
`https://raw.githubusercontent.com/layabox/LayaAir/LayaAir_3.3/src/layaAir/laya/webgl/shapes/BasePoly.ts`;
The checked-in trace also includes the unchanged `testPointInPolygon` method from
the same upstream branch's `src/layaAir/laya/utils/Utils.ts`, used by native
HitArea. Source method hashes are recorded. Tests consume the fixture without
Node or network access.

The core stroke port is derived from LayaAir (Copyright (c) 2022 layabox, MIT);
its license notice is retained at `crates/fairygui-core/LICENSE-LayaAir`.

Core tests compare native geometry for regular polygons, angle/distance changes,
concave/reversed contours, duplicates, sharp/straight edges, outlying vertices,
two-point lines and zero dimensions. Adapter tests check stable surfaces and
buffer handles, fill/stroke/ancestor alpha, shape switching and resource cleanup.
Input tests check transparent filled contours, cutout pass-through and geometry
changes. Windows smoke adds transformed concave and star-shaped controls to
Basics/Graph and verifies actual clicks, dynamic updates and disposal.

These checks do not establish native pixel parity. Laya's rasterization and
antialiasing, its exact self-intersecting fill tessellation, legacy rgba fill's
side effect of assigning the GObject alpha, and every degenerate input are not
claimed equivalent. Existing ellipse/rounded-rectangle rendering is not replaced
here. Storage-buffer materials require a Bevy rendering backend supporting that
binding; input-only hosts can still use the model and routing without RenderApp.
