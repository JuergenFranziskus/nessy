#[derive(Clone, Debug)]
pub struct Rom {
    pub header: Header,
    pub trainer: Vec<u8>,
    pub prg_rom: Vec<u8>,
    pub chr_rom: Vec<u8>,
}
impl Rom {
    pub fn parse(src: &[u8]) -> Option<Rom> {
        let header = parse_header(src)?;
        eprintln!("{:#?}", header);

        let prg_rom_size = header.prg_rom_size;
        let chr_rom_size = header.chr_rom_size;

        let trainer = if header.trainer_present {
            src[16..512 + 16].to_vec()
        } else {
            Vec::new()
        };

        let prg_rom_start = if header.trainer_present { 512 + 16 } else { 16 };
        let prg_rom_end = prg_rom_start + prg_rom_size;
        let chr_rom_end = prg_rom_end + chr_rom_size;
        let prg_rom = (&src[prg_rom_start..prg_rom_end]).to_vec();
        let chr_rom = (&src[prg_rom_end..chr_rom_end]).to_vec();

        Some(Rom {
            header,
            trainer,
            prg_rom,
            chr_rom,
        })
    }
}

fn parse_header(src: &[u8]) -> Option<Header> {
    let mut header = parse_common_header(src)?;

    if src[7] & 0xC == 0x8 {
        Some(parse_header_2(src, header)?)
    } else {
        header.adjust_sizes();
        Some(header)
    }
}
fn parse_common_header(src: &[u8]) -> Option<Header> {
    assert!(src.starts_with(b"NES\x1A"));

    let prg_rom_size = src[4];
    let chr_rom_size = src[5];

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

    let _console_type = match src[7] & 0b11 {
        0 => (), // Supported standard NES console
        _ => return None,
    };

    let header = Header {
        prg_rom_size: prg_rom_size as usize,
        chr_rom_size: chr_rom_size as usize,
        prg_ram_size: 0,
        prg_nvram_size: 0,
        chr_ram_size: 0,
        chr_nvram_size: 0,

        mirroring,
        battery_backed,
        trainer_present,
        four_screen,
        mapper: mapper as u16,
        submapper: 0,
        misc_rom_count: 0,
        default_expansion_device: 1,
    };

    Some(header)
}
fn parse_header_2(src: &[u8], mut header: Header) -> Option<Header> {
    header.mapper |= (src[8] as u16 & 0xF) << 8;
    header.submapper = src[8] >> 4;
    let prg_rom_size_high = src[9] & 0xF;
    let chr_rom_size_high = src[9] >> 4;
    header.prg_rom_size = compute_size(header.prg_rom_size as u8, prg_rom_size_high, 16 * 1024);
    header.chr_rom_size = compute_size(header.chr_rom_size as u8, chr_rom_size_high, 8 * 1024);

    let prg_ram_size_shift = (src[10] & 0xF) as usize;
    let prg_nvram_size_shift = (src[10] >> 4) as usize;
    let chr_ram_size_shift = (src[11] & 0xF) as usize;
    let chr_nvram_size_shift = (src[11] >> 4) as usize;
    header.prg_ram_size = if prg_ram_size_shift == 0 {
        0
    } else {
        64 << prg_ram_size_shift
    };
    header.prg_nvram_size = if prg_nvram_size_shift == 0 {
        0
    } else {
        64 << prg_nvram_size_shift
    };
    header.chr_ram_size = if chr_ram_size_shift == 0 {
        0
    } else {
        64 << chr_ram_size_shift
    };
    header.chr_nvram_size = if chr_nvram_size_shift == 0 {
        0
    } else {
        64 << chr_nvram_size_shift
    };
    let _timing = match src[12] & 0b11 {
        0 => (), // Supported NTSC timing
        2 => (), // Supported Multi-Region
        _ => return None,
    };
    header.misc_rom_count = src[14] & 0b11;
    header.default_expansion_device = src[15] & 0x3F;

    Some(header)
}

#[derive(Copy, Clone, Debug)]
pub struct Header {
    pub prg_rom_size: usize,
    pub chr_rom_size: usize,
    pub prg_ram_size: usize,
    pub prg_nvram_size: usize,
    pub chr_ram_size: usize,
    pub chr_nvram_size: usize,

    pub mirroring: Mirroring,
    pub battery_backed: bool,
    pub trainer_present: bool,
    pub four_screen: bool,
    pub mapper: u16,
    pub submapper: u8,
    pub misc_rom_count: u8,
    pub default_expansion_device: u8,
}
impl Header {
    fn adjust_sizes(&mut self) {
        self.prg_rom_size *= 16 * 1024;
        self.chr_rom_size *= 8 * 1024;
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Mirroring {
    Horizontal,
    Vertical,
}

fn compute_size(low: u8, high: u8, unit: usize) -> usize {
    let low = low as usize;
    let high = high as usize;

    if high == 0xF {
        let multiplier = low & 0x3;
        let exponent = low >> 2;
        2usize.pow(exponent as u32) * (multiplier * 2 + 1)
    } else {
        let count = low | (high << 8);
        count * unit
    }
}
