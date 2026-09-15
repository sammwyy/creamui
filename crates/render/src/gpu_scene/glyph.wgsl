struct ScreenUniform {
    size: vec2<f32>,
    _padding: vec2<f32>,
};

@group(0) @binding(0) var<uniform> screen: ScreenUniform;
@group(1) @binding(0) var atlas_tex: texture_2d<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;

struct GlyphInstance {
    @location(0) position: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) uv_min: vec2<f32>,
    @location(3) uv_max: vec2<f32>,
    @location(4) color: vec4<f32>,
    @location(5) clip_min: vec2<f32>,
    @location(6) clip_max: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) frag_pixel: vec2<f32>,
    @location(3) clip_min: vec2<f32>,
    @location(4) clip_max: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, instance: GlyphInstance) -> VertexOutput {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 1.0),
    );
    let corner = corners[vertex_index];
    let pixel = instance.position + corner * instance.size;
    let ndc = vec2<f32>(
        (pixel.x / screen.size.x) * 2.0 - 1.0,
        1.0 - (pixel.y / screen.size.y) * 2.0,
    );

    var out: VertexOutput;
    out.clip_position = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = mix(instance.uv_min, instance.uv_max, corner);
    out.color = instance.color;
    out.frag_pixel = pixel;
    out.clip_min = instance.clip_min;
    out.clip_max = instance.clip_max;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    if (in.frag_pixel.x < in.clip_min.x || in.frag_pixel.x > in.clip_max.x ||
        in.frag_pixel.y < in.clip_min.y || in.frag_pixel.y > in.clip_max.y) {
        discard;
    }

    let coverage = textureSample(atlas_tex, atlas_sampler, in.uv).r;
    return vec4<f32>(in.color.rgb, in.color.a * coverage);
}
