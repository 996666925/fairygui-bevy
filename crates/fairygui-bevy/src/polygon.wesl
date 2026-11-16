import bevy_ui_render::ui_vertex_output::UiVertexOutput;

struct PolygonUniform {
    canvas: vec4<f32>,
    fill: vec4<f32>,
    stroke: vec4<f32>,
    counts: vec4<u32>,
}
@group(1) @binding(0) var<uniform> material: PolygonUniform;
@group(1) @binding(1) var<storage, read> vertices: array<vec4<f32>>;

fn cross(a: vec2<f32>, b: vec2<f32>) -> f32 { return a.x*b.y-a.y*b.x; }
fn edge_contains(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, orientation: f32) -> bool {
    // Evaluate shared edges from the same origin in both triangles so their
    // rounded edge values have opposite signs. Include each shared edge once.
    let reversed = a.x > b.x || (a.x == b.x && a.y > b.y);
    let lo = select(a,b,reversed);
    let hi = select(b,a,reversed);
    let value = cross(hi-lo,p-lo)*select(orientation,-orientation,reversed);
    let edge = (b-a)*orientation;
    let top_left = edge.y < 0.0 || (edge.y == 0.0 && edge.x > 0.0);
    return value > 0.0 || (value == 0.0 && top_left);
}
fn triangle_contains(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, c: vec2<f32>) -> bool {
    let area = cross(b-a,c-a);
    if abs(area) < 0.000001 { return false; }
    let orientation = select(-1.0,1.0,area > 0.0);
    return edge_contains(p,a,b,orientation) && edge_contains(p,b,c,orientation) && edge_contains(p,c,a,orientation);
}
fn sample_color(p: vec2<f32>) -> vec4<f32> {
    if material.counts.z == 1u {
        var result = vec4<f32>(0.0);
        for (var i = 0u; i < material.counts.x; i += 1u) {
            let quad = vertices[i];
            let lo = min(quad.xy,quad.xy+quad.zw);
            let hi = max(quad.xy,quad.xy+quad.zw);
            if all(p >= lo) && all(p < hi) {
                let color = select(material.stroke,material.fill,i == 0u);
                result = vec4<f32>(color.rgb*color.a,color.a)+result*(1.0-color.a);
            }
        }
        return result;
    }
    var result = vec4<f32>(0.0);
    let count = material.counts.x;
    for (var i = 0u; i < material.counts.w; i += 1u) {
        let start = count+(material.counts.y+i)*3u;
        if triangle_contains(p,vertices[start].xy,vertices[start+1u].xy,vertices[start+2u].xy) {
            let fill = material.fill;
            result = vec4<f32>(fill.rgb*fill.a,fill.a)+result*(1.0-fill.a);
        }
    }
    for (var i = 0u; i < material.counts.y; i += 1u) {
        let start = count+i*3u;
        if triangle_contains(p,vertices[start].xy,vertices[start+1u].xy,vertices[start+2u].xy) {
            let stroke = material.stroke;
            result = vec4<f32>(stroke.rgb*stroke.a,stroke.a)+result*(1.0-stroke.a);
        }
    }
    return result;
}
@fragment
fn fragment(input: UiVertexOutput) -> @location(0) vec4<f32> {
    let p = material.canvas.xy+input.uv*material.canvas.zw;
    let dx = dpdx(p)*0.25;
    let dy = dpdy(p)*0.25;
    // Preserve native triangle overdraw; half-open edges avoid seams between
    // adjacent triangles. Derivatives keep AA stable under UI transforms.
    let color = (sample_color(p-dx-dy)+sample_color(p+dx-dy)+sample_color(p-dx+dy)+sample_color(p+dx+dy))*0.25;
    if color.a <= 0.0 { discard; }
    return vec4<f32>(color.rgb/color.a,color.a);
}
