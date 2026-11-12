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
fn triangle_contains(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, c: vec2<f32>) -> bool {
    let area = cross(b-a,c-a);
    if abs(area) < 0.000001 { return false; }
    let x = cross(b-a,p-a); let y = cross(c-b,p-b); let z = cross(a-c,p-c);
    return (x >= 0.0 && y >= 0.0 && z >= 0.0) || (x <= 0.0 && y <= 0.0 && z <= 0.0);
}
fn sample_color(p: vec2<f32>) -> vec4<f32> {
    var inside = false;
    let count = material.counts.x;
    var previous = vertices[count-1u].xy;
    for (var i = 0u; i < count; i += 1u) {
        let current = vertices[i].xy;
        if ((current.y > p.y) != (previous.y > p.y)) {
            let x = (previous.x-current.x)*(p.y-current.y)/(previous.y-current.y)+current.x;
            if p.x < x { inside = !inside; }
        }
        previous = current;
    }
    var fill = vec4<f32>(0.0);
    if inside { fill = vec4<f32>(material.fill.rgb*material.fill.a,material.fill.a); }
    for (var i = 0u; i < material.counts.y; i += 1u) {
        let start = count+i*3u;
        if triangle_contains(p,vertices[start].xy,vertices[start+1u].xy,vertices[start+2u].xy) {
            let stroke = material.stroke;
            return vec4<f32>(stroke.rgb*stroke.a,stroke.a)+fill*(1.0-stroke.a);
        }
    }
    return fill;
}
@fragment
fn fragment(input: UiVertexOutput) -> @location(0) vec4<f32> {
    let p = material.canvas.xy+input.uv*material.canvas.zw;
    let dx = dpdx(p)*0.25;
    let dy = dpdy(p)*0.25;
    // Union coverage is sampled before alpha compositing, avoiding seams between
    // adjacent stroke triangles. Derivatives keep AA stable under UI transforms.
    let color = (sample_color(p-dx-dy)+sample_color(p+dx-dy)+sample_color(p-dx+dy)+sample_color(p+dx+dy))*0.25;
    if color.a <= 0.0 { discard; }
    return vec4<f32>(color.rgb/color.a,color.a);
}
