

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    var vertices = array<vec2f, 4>(
        vec2(-1.0, -1.0),
        vec2(-1.0,  1.0),
        vec2( 1.0, -1.0),
        vec2( 1.0,  1.0),
    );

    var indices = array<u32, 6>(
        0, 1, 2,
        1, 3, 2,
    );

    let index = indices[i];
    let vertex = vertices[index];

    return vec4(vertex, 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) pixel: vec4f) -> @location(0) vec4<f32> {
    return vec4(1.0, 0.0, 0.0, 1.0);
}
