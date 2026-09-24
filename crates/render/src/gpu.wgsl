struct Globals {
    viewport: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var clips: texture_2d<f32>;
@group(1) @binding(0) var atlas: texture_2d<f32>;
@group(1) @binding(1) var image_sampler: sampler;
@group(2) @binding(0) var image: texture_2d<f32>;

const KIND_QUAD: f32 = 0.0;
const KIND_LINE: f32 = 1.0;
const KIND_GLYPH: f32 = 2.0;
const KIND_GRADIENT_QUAD: f32 = 4.0;

const KIND_BITS: u32 = 4u;
const CLIPS_PER_ROW: u32 = 256u;
const TEXELS_PER_CLIP: u32 = 3u;

struct Instance {
    @location(0) bounds: vec4<f32>,
    @location(1) data: vec4<f32>,
    @location(2) color: vec4<f32>,
    @location(3) border_color: vec4<f32>,
    @location(4) shape: vec3<f32>,
    @location(5) tag: u32,
};

struct Varyings {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) bounds: vec4<f32>,
    @location(2) @interpolate(flat) color: vec4<f32>,
    @location(3) @interpolate(flat) border_color: vec4<f32>,
    @location(4) @interpolate(flat) data: vec4<f32>,
    @location(5) @interpolate(flat) clip: vec4<f32>,
    @location(6) @interpolate(flat) rounded_clip: vec4<f32>,
    @location(7) @interpolate(flat) params: vec4<f32>,
};

fn premultiply(color: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(color.rgb * color.a, color.a);
}

@vertex
fn vs(@builtin(vertex_index) index: u32, instance: Instance) -> Varyings {
    let kind = f32(instance.tag & ((1u << KIND_BITS) - 1u));
    let clip_index = instance.tag >> KIND_BITS;
    let clip_texel = vec2<i32>(
        i32((clip_index % CLIPS_PER_ROW) * TEXELS_PER_CLIP),
        i32(clip_index / CLIPS_PER_ROW),
    );

    let corner = vec2<f32>(f32(index & 1u), f32((index >> 1u) & 1u));
    let b = instance.bounds;
    var pixel = mix(b.xy, b.zw, corner);
    if kind == KIND_GLYPH {
        pixel.x += (1.0 - corner.y) * (b.w - b.y) * instance.shape.z;
    }
    var out: Varyings;
    out.position = vec4<f32>(
        pixel.x / globals.viewport.x * 2.0 - 1.0,
        1.0 - pixel.y / globals.viewport.y * 2.0,
        0.0,
        1.0,
    );
    out.uv = corner;
    out.bounds = b;
    out.color = premultiply(instance.color);
    out.border_color = premultiply(instance.border_color);
    out.data = instance.data;
    out.clip = textureLoad(clips, clip_texel, 0);
    out.rounded_clip = textureLoad(clips, clip_texel + vec2<i32>(1, 0), 0);
    let clip_radius = textureLoad(clips, clip_texel + vec2<i32>(2, 0), 0).x;
    out.params = vec4<f32>(kind, instance.shape.x, instance.shape.y, clip_radius);
    return out;
}

fn rounded_rect_distance(p: vec2<f32>, b: vec4<f32>, radius: f32) -> f32 {
    let center = (b.xy + b.zw) * 0.5;
    let half_size = (b.zw - b.xy) * 0.5;
    let r = min(radius, min(half_size.x, half_size.y));
    let q = abs(p - center) - half_size + vec2<f32>(r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

fn segment_distance(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let ab = b - a;
    let t = clamp(dot(p - a, ab) / max(dot(ab, ab), 1e-6), 0.0, 1.0);
    return length(p - a - ab * t);
}

@fragment
fn fs(in: Varyings) -> @location(0) vec4<f32> {
    let p = in.position.xy;
    if p.x < in.clip.x || p.y < in.clip.y || p.x > in.clip.z || p.y > in.clip.w {
        discard;
    }
    var coverage = 1.0;
    if in.params.w > 0.0 {
        coverage = clamp(0.5 - rounded_rect_distance(p, in.rounded_clip, in.params.w), 0.0, 1.0);
    }

    let kind = in.params.x;
    var color: vec4<f32>;
    if kind == KIND_QUAD || kind == KIND_GRADIENT_QUAD {
        let outer = rounded_rect_distance(p, in.bounds, in.params.y);
        coverage *= clamp(0.5 - outer, 0.0, 1.0);
        if kind == KIND_GRADIENT_QUAD {
            let direction = in.data.zw - in.data.xy;
            let progress = clamp(
                dot(p - in.data.xy, direction) / max(dot(direction, direction), 1e-6),
                0.0,
                1.0,
            );
            color = mix(in.color, in.border_color, progress);
        } else {
            color = in.color;
        }
        let border = in.params.z;
        if border > 0.0 {
            let inset = vec4<f32>(border, border, -border, -border);
            let inner = rounded_rect_distance(p, in.bounds + inset, max(in.params.y - border, 0.0));
            let ring = in.border_color * clamp(0.5 + inner, 0.0, 1.0);
            color = ring + color * (1.0 - ring.a);
        }
    } else if kind == KIND_LINE {
        let distance = segment_distance(p, in.data.xy, in.data.zw) - in.params.z * 0.5;
        coverage *= clamp(0.5 - distance, 0.0, 1.0);
        color = in.color;
    } else if kind == KIND_GLYPH {
        let size = in.data.zw;
        let texel = in.data.xy + min(floor(in.uv * size), size - vec2<f32>(1.0));
        coverage *= textureLoad(atlas, vec2<i32>(texel), 0).r;
        color = in.color;
    } else {
        let sampled = textureSampleLevel(image, image_sampler, in.uv, 0.0);
        if in.color.a > 0.0 {
            color = in.color * sampled.a;
        } else {
            color = sampled;
        }
    }
    return color * coverage;
}
