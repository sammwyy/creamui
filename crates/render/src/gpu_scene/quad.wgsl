struct ScreenUniform {
    size: vec2<f32>,
    _padding: vec2<f32>,
};

@group(0) @binding(0) var<uniform> screen: ScreenUniform;

struct QuadInstance {
    @location(0) position: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) border_color: vec4<f32>,
    @location(4) corner_radius: f32,
    @location(5) border_width: f32,
    @location(6) clip_min: vec2<f32>,
    @location(7) clip_max: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_pos: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) border_color: vec4<f32>,
    @location(3) size: vec2<f32>,
    @location(4) corner_radius: f32,
    @location(5) border_width: f32,
    @location(6) frag_pixel: vec2<f32>,
    @location(7) clip_min: vec2<f32>,
    @location(8) clip_max: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, instance: QuadInstance) -> VertexOutput {
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
    out.local_pos = corner * instance.size;
    out.color = instance.color;
    out.border_color = instance.border_color;
    out.size = instance.size;
    out.corner_radius = instance.corner_radius;
    out.border_width = instance.border_width;
    out.frag_pixel = pixel;
    out.clip_min = instance.clip_min;
    out.clip_max = instance.clip_max;
    return out;
}

fn rounded_rect_sdf(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let q = abs(p) - half_size + vec2<f32>(radius, radius);
    return length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    if (in.frag_pixel.x < in.clip_min.x || in.frag_pixel.x > in.clip_max.x ||
        in.frag_pixel.y < in.clip_min.y || in.frag_pixel.y > in.clip_max.y) {
        discard;
    }

    let half_size = in.size * 0.5;
    let centered = in.local_pos - half_size;
    let radius = min(in.corner_radius, min(half_size.x, half_size.y));
    let outer_distance = rounded_rect_sdf(centered, half_size, radius);

    let edge_softness = 1.0;
    let outer_alpha = 1.0 - smoothstep(-edge_softness, edge_softness, outer_distance);

    var out_color = in.color;
    if (in.border_width > 0.0) {
        let inner_distance = outer_distance + in.border_width;
        let border_mix = smoothstep(-edge_softness, edge_softness, inner_distance);
        out_color = mix(in.color, in.border_color, border_mix);
    }

    return vec4<f32>(out_color.rgb, out_color.a * outer_alpha);
}
