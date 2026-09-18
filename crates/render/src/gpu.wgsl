struct Globals {
    viewport: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var image_sampler: sampler;
@group(1) @binding(0) var image: texture_2d<f32>;

const KIND_QUAD: f32 = 0.0;
const KIND_LINE: f32 = 1.0;
const KIND_GLYPH: f32 = 2.0;
const KIND_GRADIENT_QUAD: f32 = 4.0;

struct Instance {
    @location(0) bounds: vec4<f32>,
    @location(1) color: vec4<f32>,
    @location(2) border_color: vec4<f32>,
    @location(3) data: vec4<f32>,
    @location(4) clip: vec4<f32>,
    @location(5) rounded_clip: vec4<f32>,
    @location(6) params: vec4<f32>,
    @location(7) extra: vec4<f32>,
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

@vertex
fn vs(@builtin(vertex_index) index: u32, instance: Instance) -> Varyings {
    let corner = vec2<f32>(f32(index & 1u), f32((index >> 1u) & 1u));
    let b = instance.bounds;
    var pixel = mix(b.xy, b.zw, corner);
    if instance.params.x == KIND_GLYPH {
        pixel.x += (1.0 - corner.y) * (b.w - b.y) * instance.extra.x;
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
    out.color = instance.color;
    out.border_color = instance.border_color;
    out.data = instance.data;
    out.clip = instance.clip;
    out.rounded_clip = instance.rounded_clip;
    out.params = instance.params;
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
