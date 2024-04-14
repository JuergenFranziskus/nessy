pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 240;
pub const PIXELS: usize = WIDTH * HEIGHT;

// Each u32 stores four horizontally adjacent pixels, each pixel taking 8 bits.
// Lower-order bits corresponds to more-left pixels.
pub struct PixelBuffer(pub [u32; PIXELS]);
impl PixelBuffer {
    pub fn new() -> Self {
        Self([0; PIXELS])
    }

    pub fn set_color(&mut self, x: usize, y: usize, color: u8) {
        assert!(x < WIDTH);
        assert!(y < HEIGHT);

        let pixel_i = y * WIDTH + x;
        self.0[pixel_i] = color as u32;
    }
}
