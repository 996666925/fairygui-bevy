struct Uniform {
    screen: vec4<f32>,
    rows: array<vec4<f32>, 4>,
    offsets: vec4<f32>,
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
    let alpha = filtered.a * effect.screen.z;
    return vec4(linear(filtered.rgb) * alpha, alpha);
}
