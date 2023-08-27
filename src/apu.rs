use crate::nesbus::CpuBus;

pub struct Apu {
    dmc: Dmc,
    status: Status,
    dma: Dma,
}
impl Apu {
    pub fn init() -> Self {
        Self {
            dmc: Dmc::init(),
            status: Status::init(),
            dma: Dma::init(),
        }
    }

    pub fn cycle(&mut self, cpu: &mut CpuBus) {
        self.perform_dma(cpu);
        self.update_dmc();
        self.handle_cpu(cpu);
        self.dma.tick_counters();
    }

    fn perform_dma(&mut self, cpu: &mut CpuBus) {
        self.dma.perform_dma(cpu);

        if self.dma.dmc_dma == DmcDma::ToReceive {
            self.dmc.sample_buffer = Some(cpu.data());
            self.dma.dmc_dma = DmcDma::Idle;
        }
    }
    fn update_dmc(&mut self) {
        self.update_dmc_output();
        self.update_dmc_dma();
    }
    fn update_dmc_output(&mut self) {
        if self.dmc.cycles_until_next != 0 {
            self.dmc.cycles_until_next -= 1;
            return;
        }
        self.dmc.cycles_until_next = self.dmc.wait_cycles;

        if self.dmc.bits_remaining == 0 {
            if let Some(sample) = self.dmc.sample_buffer.take() {
                self.dmc.silence = false;
                self.dmc.sample_shifter = sample;
            } else {
                self.dmc.silence = true;
            }
            self.dmc.bits_remaining = 8;
        }

        if !self.dmc.silence {
            let bit = self.dmc.sample_shifter & 1 != 0;
            let delta = if bit { 2 } else { -2 };
            let sample = self.dmc.sample;
            let sample = sample.saturating_add_signed(delta);
            let sample = sample.min(127);
            self.dmc.sample = sample;
        }

        self.dmc.sample_shifter >>= 1;
        self.dmc.bits_remaining -= 1;
    }
    fn update_dmc_dma(&mut self) {
        if self.dmc.sample_buffer.is_some() {
            return;
        };
        if self.dma.dmc_dma != DmcDma::Idle {
            return;
        };
        if self.dmc.bytes_remaining == 0 {
            return;
        };

        let addr = self.dmc.start + self.dmc.byte_offset;
        self.dma.start_dmc_dma(addr);
        self.dmc.byte_offset += 1;
        self.dmc.bytes_remaining -= 1;

        if self.dmc.bytes_remaining == 0 {
            self.status.dmc_irq |= self.dmc.irq_enable;

            if self.dmc.loop_playback {
                self.dmc.bytes_remaining = self.dmc.length;
                self.dmc.byte_offset = 0;
            }
        }
    }

    fn handle_cpu(&mut self, cpu: &mut CpuBus) {
        match cpu.address() {
            0x4010 => {
                if cpu.read() {
                    return;
                };
                let data = cpu.data();
                self.dmc.irq_enable = data & 128 != 0;
                self.dmc.loop_playback = data & 64 != 0;
                let freq = data & 0xF;
                self.dmc.wait_cycles = wait_cycles(freq);
            }
            0x4011 => {
                if cpu.read() {
                    return;
                };
                self.dmc.sample = cpu.data() & 0x8F;
            }
            0x4012 => {
                if cpu.read() {
                    return;
                };
                self.dmc.start = (cpu.data() as u16) * 64 + 0xC000;
            }
            0x4013 => {
                if cpu.read() {
                    return;
                };
                self.dmc.length = (cpu.data() as u16) * 16 + 1;
            }
            0x4015 => {
                if cpu.read() {
                    let dmc_active = self.dmc.bytes_remaining != 0;
                    let dmc_active = if dmc_active { 1 << 4 } else { 0 };
                    let dmc_irq = (self.status.dmc_irq as u8) << 6;
                    let frame_irq = (self.status.frame_irq as u8) << 7;
                    let byte = dmc_active | dmc_irq | frame_irq;
                    cpu.set_data(byte);
                    self.status.frame_irq = false;
                } else {
                    let data = cpu.data();
                    self.status.pulse_enable[0] = data & 1 != 0;
                    self.status.pulse_enable[1] = data & 2 != 0;
                    self.status.triangle_enable = data & 4 != 0;
                    self.status.noise_enable = data & 8 != 0;

                    self.status.dmc_irq = false;
                    let d = data & 16 != 0;
                    if d {
                        self.dmc.bytes_remaining = self.dmc.length;
                        self.dmc.byte_offset = 0;
                    } else {
                        self.dmc.bytes_remaining = 0;
                    }
                }
            }
            0x4014 => {
                if cpu.read() {
                    return;
                };
                self.dma.start_oam_dma(cpu.data());
            }
            _ => (),
        }
    }
}

fn wait_cycles(freq: u8) -> u16 {
    static CYCLES: [u16; 16] = [
        428, 380, 340, 320, 286, 254, 226, 214, 190, 160, 142, 128, 106, 84, 72, 54,
    ];
    CYCLES[freq as usize]
}

struct Dmc {
    irq_enable: bool,
    loop_playback: bool,
    wait_cycles: u16,
    cycles_until_next: u16,

    sample: u8,
    start: u16,
    length: u16,

    bytes_remaining: u16,
    byte_offset: u16,

    bits_remaining: u8,
    sample_buffer: Option<u8>,
    sample_shifter: u8,
    silence: bool,
}
impl Dmc {
    fn init() -> Self {
        Self {
            irq_enable: false,
            loop_playback: false,
            wait_cycles: 54,
            cycles_until_next: 0,

            sample: 0,
            start: 0,
            length: 0,

            bytes_remaining: 0,
            byte_offset: 0,

            bits_remaining: 0,
            sample_buffer: None,
            sample_shifter: 0,
            silence: true,
        }
    }
}

struct Status {
    pulse_enable: [bool; 2],
    triangle_enable: bool,
    noise_enable: bool,

    dmc_irq: bool,
    frame_irq: bool,
}
impl Status {
    fn init() -> Self {
        Self {
            pulse_enable: [false; 2],
            triangle_enable: false,
            noise_enable: false,
            dmc_irq: false,
            frame_irq: false,
        }
    }
}

struct Dma {
    put_cycle: bool,

    oam_dma: OamDma,
    oam_page: u8,
    oam_step: u8,

    dmc_dma: DmcDma,
    dmc_address: u16,
}
impl Dma {
    fn init() -> Self {
        Self {
            put_cycle: false,

            oam_dma: OamDma::Idle,
            oam_page: 0,
            oam_step: 0,

            dmc_dma: DmcDma::Idle,
            dmc_address: 0,
        }
    }

    fn perform_dma(&mut self, cpu: &mut CpuBus) {
        cpu.set_not_ready(false);
        let halt_oam = self.perform_dmc_dma(cpu);

        if halt_oam {
            if self.oam_dma == OamDma::ToRead {
                self.oam_dma = OamDma::Align;
            }
            return;
        }
        match self.oam_dma {
            OamDma::Idle => (),
            OamDma::Started => {
                cpu.set_not_ready(true);
                if !cpu.halt() {
                    return;
                };
                if self.put_cycle {
                    return;
                };
                cpu.set_read(true);
                cpu.set_address(self.oam_addr());
                self.oam_dma = OamDma::ToWrite;
            }
            OamDma::ToWrite => {
                cpu.set_not_ready(true);
                cpu.set_read(false);
                cpu.set_address(0x2004);
                let done = self.oam_step == 255;
                self.oam_dma = if done { OamDma::Idle } else { OamDma::ToRead };
                self.oam_step = self.oam_step.wrapping_add(1);
            }
            OamDma::ToRead => {
                cpu.set_not_ready(true);
                cpu.set_address(self.oam_addr());
                cpu.set_read(true);
                self.oam_dma = OamDma::ToWrite;
            }
            OamDma::Align => {
                cpu.set_not_ready(true);
                self.oam_dma = OamDma::ToRead;
            }
        }
    }
    fn perform_dmc_dma(&mut self, cpu: &mut CpuBus) -> bool {
        match self.dmc_dma {
            DmcDma::Idle => false,
            DmcDma::Started => {
                cpu.set_not_ready(true);
                if !cpu.halt() {
                    return false;
                };
                if self.put_cycle {
                    return false;
                };
                self.dmc_dma = DmcDma::Dummy;
                false
            }
            DmcDma::Dummy => {
                cpu.set_not_ready(true);
                self.dmc_dma = DmcDma::ToRead;
                false
            }
            DmcDma::ToRead => {
                cpu.set_not_ready(true);
                cpu.set_address(self.dmc_address);
                cpu.set_read(true);
                self.dmc_dma = DmcDma::ToReceive;
                true
            }
            DmcDma::ToReceive => false,
        }
    }

    fn start_oam_dma(&mut self, page: u8) {
        self.oam_dma = OamDma::Started;
        self.oam_page = page;
        self.oam_step = 0;
    }
    fn start_dmc_dma(&mut self, address: u16) {
        self.dmc_dma = DmcDma::Started;
        self.dmc_address = address;
    }

    fn tick_counters(&mut self) {
        self.put_cycle = !self.put_cycle
    }

    fn oam_addr(&self) -> u16 {
        let low = self.oam_step as u16;
        let high = (self.oam_page as u16) << 8;
        low | high
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
enum DmcDma {
    Idle,
    Started,
    Dummy,
    ToRead,
    ToReceive,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
enum OamDma {
    Idle,
    Started,
    ToRead,
    ToWrite,
    Align,
}
