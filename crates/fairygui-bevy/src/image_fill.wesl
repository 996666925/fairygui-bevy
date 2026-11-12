import bevy_ui_render::ui_vertex_output::UiVertexOutput;
import bevy_render::color_operations::{linear_rgb_to_srgb,srgb_to_linear_rgb};

struct FillUniform {
    points: array<vec4<f32>,12>,
    domain: vec4<f32>,
    atlas: vec4<f32>,
    source_slices: vec4<f32>,
    target_slices: vec4<f32>,
    repeat: vec4<f32>,
    grid_u: vec4<f32>,
    grid_v: vec4<f32>,
    clip: vec4<f32>,
    tint: vec4<f32>,
    color_rows: array<vec4<f32>,4>,
    color_offset: vec4<f32>,
    effects: vec4<u32>,
    flags: vec4<u32>,
}
@group(1) @binding(0) var<uniform> material: FillUniform;
@group(1) @binding(1) var image: texture_2d<f32>;
@group(1) @binding(2) var image_sampler: sampler;

fn axis(p:f32,low:f32,high:f32,source_low:f32,source_high:f32,repeats:f32)->f32 {
    if p<low {return p/low*source_low;}
    if p>high {return source_high+(p-high)/(1.0-high)*(1.0-source_high);}
    return source_low+fract(repeats*(p-low)/max(high-low,0.000001))*(source_high-source_low);
}
fn image_uv(uv:vec2<f32>)->vec2<f32> {
    let s=material.source_slices;let t=material.target_slices;
    var r=material.repeat.xy;
    if uv.x>=t.x && uv.y>=t.y && uv.x<=t.z && uv.y<=t.w {r=material.repeat.zw;}
    if material.flags.w!=0u {
        return vec2<f32>(native_axis(uv.x,t.x,t.z,material.grid_u,r.x),native_axis(uv.y,t.y,t.w,material.grid_v,r.y));
    }
    let mapped=vec2<f32>(axis(uv.x,t.x,t.z,s.x,s.z,r.x),axis(uv.y,t.y,t.w,s.y,s.w,r.y));
    return material.atlas.xy+mapped*(material.atlas.zw-material.atlas.xy);
}
fn native_axis(p:f32,low:f32,high:f32,source:vec4<f32>,repeats:f32)->f32 {
    if p<=low {
        if low<=0.0 {return source.y;}
        return source.x+p/low*(source.y-source.x);
    }
    if p>=high {
        if high>=1.0 {return source.z;}
        return source.z+(p-high)/(1.0-high)*(source.w-source.z);
    }
    return source.y+fract(repeats*(p-low)/max(high-low,0.000001))*(source.z-source.y);
}
fn inside(p:vec2<f32>)->f32 {
    let count=material.flags.x;
    if count<3u {return 0.0;}
    var last=material.points[count-1u].xy;
    var hit=false;
    for(var i=0u;i<count;i+=1u) {
        let current=material.points[i].xy;
        if (last.y>p.y)!=(current.y>p.y) {
            if p.x<(last.x-current.x)*(p.y-current.y)/(last.y-current.y)+current.x {hit=!hit;}
        }
        last=current;
    }
    return select(0.0,1.0,hit);
}
// FairyGUI/Laya matrices operate on encoded channels, not Bevy's linear
// texture samples. Output stays straight alpha for Bevy's UI blend state.
fn filtered(sample:vec4<f32>)->vec4<f32> {
    if material.effects.x==0u {return sample*material.tint;}
    let c=vec4<f32>(linear_rgb_to_srgb(sample.rgb)*linear_rgb_to_srgb(material.tint.rgb),sample.a*material.tint.a);
    let rows=material.color_rows;
    let result=clamp(vec4<f32>(dot(rows[0],c),dot(rows[1],c),dot(rows[2],c),dot(rows[3],c))+material.color_offset,vec4<f32>(0.0),vec4<f32>(1.0));
    return vec4<f32>(srgb_to_linear_rgb(result.rgb),result.a);
}
@fragment
fn fragment(input:UiVertexOutput)->@location(0) vec4<f32> {
    let uv=vec2<f32>(select(input.uv.x,1.0-input.uv.x,material.flags.y!=0u),select(input.uv.y,1.0-input.uv.y,material.flags.z!=0u));
    // Sample before contour branches, keeping derivatives defined for filtering.
    let atlas_uv=image_uv(uv);
    let color=filtered(textureSample(image,image_sampler,atlas_uv));
    if material.flags.w!=0u && (any(atlas_uv<material.clip.xy) || any(atlas_uv>material.clip.zw)) {discard;}
    let p=material.domain.xy+uv*material.domain.zw;
    let dx=dpdx(p)*0.25;let dy=dpdy(p)*0.25;
    let coverage=(inside(p-dx-dy)+inside(p+dx-dy)+inside(p-dx+dy)+inside(p+dx+dy))*0.25;
    if coverage==0.0 {discard;}
    return vec4<f32>(color.rgb,color.a*coverage);
}
