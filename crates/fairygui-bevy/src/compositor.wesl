struct Uniform {
    screen: vec4<f32>,
    rows: array<vec4<f32>, 4>,
    offsets: vec4<f32>,
    clip_rect: vec4<f32>,
    clip_edges: vec4<f32>,
    clip_x: vec4<f32>,
    clip_y: vec4<f32>,
}
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;
@group(0) @binding(2) var<uniform> effect: Uniform;
@group(0) @binding(3) var alpha_mask: texture_2d<f32>;

fn encoded(c: vec3<f32>) -> vec3<f32> {
    return select(c * 12.92, 1.055 * pow(max(c, vec3(0.)), vec3(1. / 2.4)) - 0.055, c > vec3(0.0031308));
}
fn linear(c: vec3<f32>) -> vec3<f32> {
    return select(c / 12.92, pow((c + 0.055) / 1.055, vec3(2.4)), c > vec3(0.04045));
}
struct Input {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}
@fragment
fn fragment(in: Input) -> @location(0) vec4<f32> {
    // UI renders straight source colors with alpha blending into a transparent
    // target. The accumulated target therefore contains premultiplied RGB.
    var sample = textureSample(source, source_sampler, in.position.xy / effect.screen.xy);
    if effect.screen.w > 0. {
        sample *= textureSample(alpha_mask,source_sampler,in.position.xy / effect.screen.xy).a;
    }
    if sample.a <= 0. { return vec4(0.); }
    let color = vec4(encoded(sample.rgb / sample.a), sample.a);
    let filtered = clamp(vec4(dot(effect.rows[0],color), dot(effect.rows[1],color), dot(effect.rows[2],color), dot(effect.rows[3],color)) + effect.offsets, vec4(0.), vec4(1.));
    var fade = 1.;
    if effect.clip_x.w > 0. {
        let p = vec3(in.position.xy, 1.);
        let local = vec2(dot(effect.clip_x.xyz, p), dot(effect.clip_y.xyz, p));
        let distances = vec4(local - effect.clip_rect.xy, effect.clip_rect.zw - local);
        if any(distances < vec4(0.)) { return vec4(0.); }
        // FairyGUI selects each axis's leading/trailing fade at its midpoint.
        let leading = local <= (effect.clip_rect.xy + effect.clip_rect.zw) * 0.5;
        let distance = select(distances.zw, distances.xy, leading);
        let widths = select(effect.clip_edges.zw, effect.clip_edges.xy, leading);
        let edges = select(vec2(1.), clamp(distance / max(widths, vec2(0.0001)), vec2(0.), vec2(1.)), widths > vec2(0.));
        fade = min(edges.x, edges.y);
    }
    let alpha = filtered.a * effect.screen.z * fade;
    return vec4(linear(filtered.rgb) * alpha, alpha);
}
