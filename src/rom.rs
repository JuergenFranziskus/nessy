use std::{error::Error, fmt::Display, ops::Range};

#[derive(Clone, Debug)]
pub struct Rom {
    pub bytes: Vec<u8>,
    pub header: Header,
    pub trainer: Range<usize>,
    pub prg_rom: Range<usize>,
    pub chr_rom: Range<usize>,
    pub msc_rom: Range<usize>,
}
impl Rom {
    pub fn parse(src: Vec<u8>) -> Result<Self, RomErr> {
        let header = Header::parse(&src)?;

        let trainer_start = 16;
        let trainer_len = if header.trainer_present { 512 } else { 0 };
        let trainer_end = trainer_start + trainer_len;
        if trainer_end > src.len() {
            return Err(RomErr::TrainerIncomplete);
        }

        let trainer = trainer_start..trainer_end;

        let prg_rom_start = trainer_end;
        let prg_rom_end = prg_rom_start + header.prg_rom_size as usize;
        if prg_rom_end > src.len() {
            return Err(RomErr::PrgRomIncomplete);
        }
        let prg_rom = prg_rom_start..prg_rom_end;

        let chr_rom_start = prg_rom_end;
        let chr_rom_end = chr_rom_start + header.chr_rom_size as usize;
        if chr_rom_end > src.len() {
            return Err(RomErr::ChrRomIncomplete);
        }
        let chr_rom = chr_rom_start..chr_rom_end;

        let msc_rom = chr_rom_end..src.len();

        Ok(Self {
            bytes: src,
            header,
            trainer,
            prg_rom,
            chr_rom,
            msc_rom,
        })
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Header {
    pub nes2: bool,
    pub prg_rom_size: u64,
    pub prg_ram_size: u32,
    pub prg_nvram_size: u32,
    pub chr_rom_size: u64,
    pub chr_ram_size: u32,
    pub chr_nvram_size: u32,
    pub mapper: u16,
    pub submapper: u8,
    pub vertical_mirroring: bool,
    pub battery_present: bool,
    pub trainer_present: bool,
    pub four_screen_mode: bool,
    pub console_type: ConsoleType,
    pub timing: Timing,
    pub misc_roms: u8,
}
impl Header {
    pub fn parse(src: &[u8]) -> Result<Self, RomErr> {
        if src.len() < 16 {
            return Err(RomErr::HeaderIncomplete);
        }
        if !src.starts_with(&[b'N', b'E', b'S', 0x1A]) {
            return Err(RomErr::WrongMagicNumber);
        }
        let is_nes2 = src[7] & 0xC == 0x8;

        let prg_rom_size_low = src[4];
        let chr_rom_size_low = src[5];

        let vertical_mirroring = src[6] & 1 != 0;
        let battery_present = src[6] & 2 != 0;
        let trainer_present = src[6] & 4 != 0;
        let four_screen_mode = src[6] & 8 != 0;

        let mapper_low = src[6] >> 4;
        let console_type = src[7] & 3;
        let mapper = (mapper_low | (src[7] & 0xF0)) as u16;

        if !is_nes2 {
            return Ok(Self {
                nes2: false,
                prg_rom_size: (prg_rom_size_low as u64) * 16384,
                prg_ram_size: 0,
                prg_nvram_size: 0,
                chr_rom_size: (chr_rom_size_low as u64) * 8192,
                chr_ram_size: if chr_rom_size_low == 0 { 8192 } else { 0 },
                chr_nvram_size: 0,
                mapper,
                submapper: 0,
                vertical_mirroring,
                battery_present,
                trainer_present,
                four_screen_mode,
                console_type: ConsoleType::from_ines(console_type)?,
                timing: Timing::Ntsc,
                misc_roms: 0,
            });
        }

        let mapper_high = (src[8] as u16 & 0xF) << 8;
        let mapper = mapper | mapper_high;
        let submapper = src[8] >> 4;

        let prg_rom_size_high = src[9] & 0xF;
        let chr_rom_size_high = src[9] >> 4;
        let prg_rom_size = calc_size(prg_rom_size_low, prg_rom_size_high, 16384);
        let chr_rom_size = calc_size(chr_rom_size_low, chr_rom_size_high, 8192);

        let prg_ram_shift_count = src[10] & 0xF;
        let prg_nvram_shift_count = src[10] >> 4;
        let prg_ram_size = if prg_ram_shift_count == 0 {
            0
        } else {
            64 << prg_ram_shift_count as u32
        };
        let prg_nvram_size = if prg_nvram_shift_count == 0 {
            0
        } else {
            64 << prg_nvram_shift_count as u32
        };

        let chr_ram_shift_count = src[11] & 0xF;
        let chr_nvram_shift_count = src[11] >> 4;
        let chr_ram_size = if chr_ram_shift_count == 0 {
            0
        } else {
            64 << chr_ram_shift_count as u32
        };
        let chr_nvram_size = if chr_nvram_shift_count == 0 {
            0
        } else {
            64 << chr_nvram_shift_count as u32
        };

        let timing = match src[12] & 3 {
            0 => Timing::Ntsc,
            1 => Timing::Pal,
            2 => Timing::Multi,
            3 => Timing::Ntsc,
            _ => unreachable!(),
        };

        let console_type = parse_console_type(console_type, src[13]);
        let misc_roms = src[14] & 3;

        Ok(Self {
            nes2: true,
            prg_rom_size,
            prg_ram_size,
            prg_nvram_size,
            chr_rom_size,
            chr_ram_size,
            chr_nvram_size,
            mapper,
            submapper,
            vertical_mirroring,
            battery_present,
            trainer_present,
            four_screen_mode,
            console_type,
            timing,
            misc_roms,
        })
    }
}

fn calc_size(low: u8, high: u8, unit: u64) -> u64 {
    let low = low as u64;
    let high = high as u64;

    if high < 0xF {
        (low | high << 8) * unit
    } else {
        let mul = low & 3;
        let exp = low >> 2;
        2u64.pow(exp as u32) * (2 * mul + 1)
    }
}
fn parse_console_type(b7: u8, b13: u8) -> ConsoleType {
    match b7 {
        0 => ConsoleType::Nes,
        1 => {
            let ppu = match b13 & 0xF {
                0 => VsPpu::RP2C03B,
                1 => VsPpu::RP2C03G,
                2 => VsPpu::RP2C04_0001,
                3 => VsPpu::RP2C04_0002,
                4 => VsPpu::RP2C04_0003,
                5 => VsPpu::RP2C04_0004,
                6 => VsPpu::RC2C03B,
                7 => VsPpu::RC2C03C,
                8 => VsPpu::RC2C05_01,
                9 => VsPpu::RC2C05_02,
                10 => VsPpu::RC2C05_03,
                11 => VsPpu::RC2C05_04,
                12 => VsPpu::RC2C05_05,
                _ => unreachable!(),
            };
            let hardware = match b13 >> 4 {
                0 => VsHardware::Uni,
                1 => VsHardware::UniRBI,
                2 => VsHardware::UniTKO,
                3 => VsHardware::UniSuperXevious,
                4 => VsHardware::UniIceClimber,
                5 => VsHardware::Dual,
                6 => VsHardware::DualRaidOnBungelingBay,
                _ => unreachable!(),
            };
            ConsoleType::Vs(ppu, hardware)
        }
        2 => ConsoleType::Playchoice,
        3 => match b13 & 0xF {
            3 => ConsoleType::Famiclone,
            4 => ConsoleType::NesEPSM,
            5 => ConsoleType::VT01,
            6 => ConsoleType::VT02,
            7 => ConsoleType::VT03,
            8 => ConsoleType::VT09,
            9 => ConsoleType::VT32,
            10 => ConsoleType::VT369,
            11 => ConsoleType::UMC,
            12 => ConsoleType::FamicomNetworkSystem,
            _ => unreachable!(),
        },
        _ => unreachable!(),
    }
}

#[derive(Copy, Clone, Debug)]
pub enum ConsoleType {
    Nes,
    Vs(VsPpu, VsHardware),
    Playchoice,
    Famiclone,
    NesEPSM,
    VT01,
    VT02,
    VT03,
    VT09,
    VT32,
    VT369,
    UMC,
    FamicomNetworkSystem,
}
impl ConsoleType {
    pub fn from_ines(n: u8) -> Result<Self, RomErr> {
        match n {
            0 => Ok(Self::Nes),
            1 => Ok(Self::Vs(VsPpu::RP2C03B, VsHardware::Uni)),
            2 => Ok(Self::Playchoice),
            3 => Err(RomErr::WrongINesConsoleType),
            _ => unreachable!(),
        }
    }
}
#[derive(Copy, Clone, Debug)]
pub enum VsPpu {
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
}
#[derive(Copy, Clone, Debug)]
pub enum VsHardware {
    Uni,
    UniRBI,
    UniTKO,
    UniSuperXevious,
    UniIceClimber,
    Dual,
    DualRaidOnBungelingBay,
}
#[derive(Copy, Clone, Debug)]
pub enum Timing {
    Ntsc,
    Pal,
    Multi,
    Dendy,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RomErr {
    HeaderIncomplete,
    WrongMagicNumber,
    WrongINesConsoleType,
    TrainerIncomplete,
    PrgRomIncomplete,
    ChrRomIncomplete,
}
impl Display for RomErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HeaderIncomplete => write!(f, "the rom is too small to fit the full INES header"),
            Self::WrongMagicNumber => write!(f, "the rom does not start with the INes magic number 'NES', 0x1A"),
            Self::WrongINesConsoleType => write!(f, "the rom, interpreted as legacy INes, specifies an invalid console type"),
            Self::TrainerIncomplete => write!(f, "the rom is smaller than specified by the header, starting within the trainer"),
            Self::PrgRomIncomplete => write!(f, "the rom is smaller than specified by the header, starting within the program rom"),
            Self::ChrRomIncomplete => write!(f, "the rom is smaller than specified by the header, starting within the character rom"),
        }
    }
}
impl Error for RomErr {}
