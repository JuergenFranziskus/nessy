#[derive(Clone, Debug)]
pub struct Rom {
    pub header: Header,
    pub prg_rom: Vec<u8>,
    pub chr_rom: Vec<u8>,
}
impl Rom {
    pub fn parse(src: &[u8]) -> Rom {
        assert!(src.starts_with(b"NES\x1A"));

        if src[7] & 0xC == 0x8 {
            eprintln!("Detected Nes 2.0 header");
            panic!("Nes 2.0 roms are not yet supported");
        }

        let prg_rom_size = (src[4] as usize) << 14;
        let chr_rom_size = (src[5] as usize) << 13;

        let mirror_bit = src[6] & 1 != 0;
        let mirroring = if mirror_bit {
            Mirroring::Vertical
        } else {
            Mirroring::Horizontal
        };
        let battery_backed = src[6] & 2 != 0;
        let trainer_present = src[6] & 4 != 0;
        let four_screen = src[6] & 8 != 0;

        let mapper_low = src[6] >> 4;
        let mapper_high = src[7] & 0xF0;
        let mapper = mapper_low | mapper_high;

        let header = Header {
            prg_rom_size,
            chr_rom_size,
            mirroring,
            battery_backed,
            trainer_present,
            four_screen,
            mapper: mapper as usize,
        };

        let prg_rom_start = 16;
        let prg_rom_end = prg_rom_start + prg_rom_size;
        let chr_rom_end = prg_rom_end + chr_rom_size;
        let prg_rom = (&src[prg_rom_start..prg_rom_end]).to_vec();
        let chr_rom = (&src[prg_rom_end..chr_rom_end]).to_vec();

        Rom {
            header,
            prg_rom,
            chr_rom,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Header {
    pub prg_rom_size: usize,
    pub chr_rom_size: usize,

    pub mirroring: Mirroring,
    pub battery_backed: bool,
    pub trainer_present: bool,
    pub four_screen: bool,
    pub mapper: usize,
}

#[derive(Copy, Clone, Debug)]
pub enum Mirroring {
    Horizontal,
    Vertical,
}
