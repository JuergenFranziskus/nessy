#[derive(Clone, Debug)]
pub struct Rom {
    pub header: Header,
    pub prg_rom: Vec<u8>,
    pub chr_rom: Vec<u8>,
}
impl Rom {
    pub fn parse(src: &[u8]) -> Rom {
        let header = parse_header(src);

        let prg_rom_size = header.prg_rom_size();
        let chr_rom_size = header.chr_rom_size();

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

fn parse_header(src: &[u8]) -> Header {
    let mut header = parse_common_header(src);

    if src[7] & 0xC == 0x8 {
        header.header_2 = Some(parse_header_2(src, &header));
    }

    header
}
fn parse_common_header(src: &[u8]) -> Header {
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

    let console_type = match src[7] & 0b11 {
        0 => ConsoleType::Nes,
        1 => ConsoleType::VsSystem,
        2 => ConsoleType::Playchoice,
        3 => ConsoleType::Extended,
        _ => unreachable!(),
    };

    let header = Header {
        prg_rom_size_low: prg_rom_size,
        chr_rom_size_low: chr_rom_size,
        mirroring,
        battery_backed,
        trainer_present,
        four_screen,
        console_type,
        mapper,
        header_2: None,
    };

    header
}
fn parse_header_2(src: &[u8], header: &Header) -> Header2 {
    let mapper_plane = src[8] & 0xF;
    let submapper = src[8] >> 4;
    let prg_rom_size_high = src[9] & 0xF;
    let chr_rom_size_high = src[9] >> 4;
    let prg_ram_size_shift = (src[10] & 0xF) as usize;
    let prg_nvram_size_shift = (src[10] >> 4) as usize;
    let chr_ram_size_shift = (src[11] & 0xF) as usize;
    let chr_nvram_size_shift = (src[11] >> 4) as usize;
    let prg_ram_size = if prg_ram_size_shift == 0 {
        0
    } else {
        64 << prg_ram_size_shift
    };
    let prg_nvram_size = if prg_nvram_size_shift == 0 {
        0
    } else {
        64 << prg_nvram_size_shift
    };
    let chr_ram_size = if chr_ram_size_shift == 0 {
        0
    } else {
        64 << chr_ram_size_shift
    };
    let chr_nvram_size = if chr_nvram_size_shift == 0 {
        0
    } else {
        64 << chr_nvram_size_shift
    };
    let timing = match src[12] & 0b11 {
        0 => Timing::Ntsc,
        1 => Timing::Pal,
        2 => Timing::MultiRegion,
        3 => Timing::Dendy,
        _ => unreachable!(),
    };
    let extended_console_type = ExtendedConsoleType::get(header.console_type, src[13]);
    let misc_roms = src[14] & 0b11;
    let default_expansion_device = src[15] & 0x3F;

    Header2 {
        mapper_plane,
        submapper,
        prg_rom_size_high,
        chr_rom_size_high,
        prg_ram_size,
        prg_nvram_size,
        chr_ram_size,
        chr_nvram_size,
        timing,
        extended_console_type,
        misc_roms,
        default_expansion_device,
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Header {
    pub prg_rom_size_low: u8,
    pub chr_rom_size_low: u8,

    pub mirroring: Mirroring,
    pub battery_backed: bool,
    pub trainer_present: bool,
    pub four_screen: bool,
    pub console_type: ConsoleType,
    pub mapper: u8,

    pub header_2: Option<Header2>,
}
impl Header {
    pub fn mapper(&self) -> usize {
        let mapper = self.mapper as usize;
        let plane = if let Some(h) = self.header_2 {
            h.mapper_plane as usize
        } else {
            0
        };

        mapper | plane << 8
    }

    pub fn prg_rom_size(&self) -> usize {
        let Some(header_2) = self.header_2 else {
            return (self.prg_rom_size_low) as usize * 16 * 1024;
        };

        header_2.prg_rom_size(self.prg_rom_size_low)
    }
    pub fn chr_rom_size(&self) -> usize {
        let Some(header_2) = self.header_2 else {
            return (self.chr_rom_size_low) as usize * 8 * 1024;
        };

        header_2.chr_rom_size(self.chr_rom_size_low)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Mirroring {
    Horizontal,
    Vertical,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ConsoleType {
    Nes,
    VsSystem,
    Playchoice,
    Extended,
}

#[derive(Copy, Clone, Debug)]
pub struct Header2 {
    pub mapper_plane: u8,
    pub submapper: u8,
    pub prg_rom_size_high: u8,
    pub chr_rom_size_high: u8,
    pub prg_ram_size: usize,
    pub prg_nvram_size: usize,
    pub chr_ram_size: usize,
    pub chr_nvram_size: usize,
    pub timing: Timing,
    pub extended_console_type: Option<ExtendedConsoleType>,
    pub misc_roms: u8,
    pub default_expansion_device: u8,
}
impl Header2 {
    fn prg_rom_size(&self, low: u8) -> usize {
        compute_size(low, self.prg_rom_size_high, 16 * 1024)
    }
    fn chr_rom_size(&self, low: u8) -> usize {
        compute_size(low, self.chr_rom_size_high, 8 * 1024)
    }
}

fn compute_size(low: u8, high: u8, unit: usize) -> usize {
    let low = low as usize;
    let high = high as usize;

    if high == 0xF {
        todo!()
    } else {
        let units = low | (high << 8);
        units * unit
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Timing {
    Ntsc,
    Pal,
    MultiRegion,
    Dendy,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ExtendedConsoleType {
    VsSystem(VsPPUType, VsHardwareType),
    FamicloneDecimal,
    NESEPSM,
    VRTechnologyVT01,
    VRTechnologyVT02,
    VRTechnologyVT03,
    VRTechnologyVT09,
    VRTechnologyVT32,
    VRTechnologyVT369,
    UM6578,
    FamicomNetworkSystem,
    Reserved,
}
impl ExtendedConsoleType {
    pub fn get(console_type: ConsoleType, _et_byte: u8) -> Option<Self> {
        match console_type {
            ConsoleType::Nes => None,
            ConsoleType::Playchoice => None,
            ConsoleType::VsSystem => {
                todo!()
            }
            ConsoleType::Extended => {
                todo!()
            }
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VsPPUType {
    RP2C03B,
    RP2C03G,
    RP2C04_0001,
    RP2C04_0002,
    RP2C04_0003,
    RP2C04_0004,
    RC2C03B,
    RC2C03C,
    RC2C05_01,
    RC2C05_02,
    RC2C05_03,
    RC2C05_04,
    RC2C05_05,
    Reserved,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VsHardwareType {
    VsUnisystem,
    VsUnisystemRBI,
    VsUnisystemTKO,
    VsUnisystemSuperXevious,
    VsUnisystemIceClimber,
    VsDualSystem,
    VsDualSystemRaid,
    Reserved,
}
