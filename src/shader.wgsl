

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

const NES_WIDTH: u32 = 256;
const NES_HEIGHT: u32 = 240;
const NES_PIXELS: u32 = NES_WIDTH * NES_HEIGHT;

const PALETTE_ENTRIES: u32 = 64;

@group(0) @binding(0) var<storage> pixels: array<u32, NES_PIXELS>;
@group(0) @binding(1) var<uniform> screen: vec2u;
@group(0) @binding(2) var<storage> palette: array<vec4f, PALETTE_ENTRIES>;


@fragment
fn fs_main(@builtin(position) pixel: vec4f) -> @location(0) vec4<f32> {
    let nes_pixel = nes_pixel(pixel.xy);
    if nes_pixel_oob(nes_pixel) { return vec4(0.0, 0.0, 0.0, 1.0); };
    let color = color(nes_pixel);    
    return color;
}


fn nes_pixel(screen_pixel: vec2f) -> vec2u {
    let x = screen_pixel.x / f32(screen.x) * f32(NES_WIDTH);
    let y = screen_pixel.y / f32(screen.y) * f32(NES_HEIGHT);
    return vec2u(u32(x), u32(y));
}
fn nes_pixel_oob(pixel: vec2u) -> bool {
    return pixel.x >= NES_WIDTH || pixel.y >= NES_HEIGHT;
}

fn color(pixel: vec2u) -> vec4f {
    let pixel_i = pixel.y * NES_WIDTH + pixel.x;
    let palette_i = pixels[pixel_i];
    return palette[palette_i];
}
