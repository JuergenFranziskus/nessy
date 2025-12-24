

struct VertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
};


@vertex
fn vertex_main(@builtin(vertex_index) vert_index: u32) -> VertexOut {
    const vertices = array(
        vec2(-1.0, -1.0),
        vec2(-1.0,  1.0),
        vec2( 1.0, -1.0),

        vec2(-1.0,  1.0),
        vec2( 1.0,  1.0),
        vec2( 1.0, -1.0),
    );
    const tex_coords = array(
        vec2(0.0, 0.0),
        vec2(0.0, 1.0),
        vec2(1.0, 0.0),

        vec2(0.0, 1.0),
        vec2(1.0, 1.0),
        vec2(1.0, 0.0),
    );

    let vertex = vertices[vert_index];
    let tex_coord = tex_coords[vert_index];

    var out: VertexOut;
    out.clip_pos = vec4(vertex, 0.0, 1.0);
    out.tex_coord = vec2(tex_coord.x, 1.0 - tex_coord.y);

    return out;
}

@group(0) @binding(0) var texture: texture_2d<f32>;
@group(0) @binding(1) var sample: sampler;

@fragment
fn fragment_main(in: VertexOut) -> @location(0) vec4<f32> {
    return textureSample(texture, sample, in.tex_coord);
}
