use crate::nesbus::CpuBus;

const SAMPLES_PER_SECOND: usize = 44100;
const CYCLES_PER_SAMPLE: usize = 1_789773 / SAMPLES_PER_SECOND;

pub struct Apu {
    dmc: Dmc,
    status: Status,
    dma: Dma,
    frame_counter: FrameCounter,

    cycles_since_sample: usize,
}
impl Apu {
    pub fn init() -> Self {
        Self {
            dmc: Dmc::init(),
            status: Status::init(),
            dma: Dma::init(),
            frame_counter: FrameCounter::init(),

            cycles_since_sample: 0,
        }
    }

    pub fn cycle(&mut self, cpu: &mut CpuBus) {
        self.produce_sample();
        self.update_sound_channels();
        self.tick_frame_counter();
        self.perform_dma(cpu);
        self.update_dmc();
        self.handle_cpu(cpu);
        self.assert_irqs(cpu);
        self.dma.tick_counters();
    }

    fn update_sound_channels(&mut self) {
        // An APU cycle occurs every 2 CPU cycles.
        // Repurpose dma cycle flag for fun and profit.
        if self.dma.put_cycle {
            return;
        };
    }

    fn tick_frame_counter(&mut self) {
        if self.frame_counter.cycles_until_step < FrameCounter::CYCLES_PER_STEP {
            self.frame_counter.cycles_until_step += 1;
            return;
        }
        self.frame_counter.cycles_until_step = 0;

        if self.frame_counter.mode {
            // Five step sequence
            match self.frame_counter.step {
                0 => self.tick_envelopes(),
                1 => {
                    self.tick_envelopes();
                    self.tick_length_counters()
                }
                2 => self.tick_envelopes(),
                3 => (),
                4 => {
                    self.tick_envelopes();
                    self.tick_length_counters()
                }
                5.. => unreachable!(),
            }
            self.frame_counter.step += 1;
            if self.frame_counter.step >= 5 {
                self.frame_counter.step = 0;
            }
        } else {
            // Four step sequence
            match self.frame_counter.step {
                0 => self.tick_envelopes(),
                1 => {
                    self.tick_envelopes();
                    self.tick_length_counters()
                }
                2 => self.tick_envelopes(),
                3 => {
                    self.tick_envelopes();
                    self.tick_length_counters();
                    self.status.frame_irq |= !self.frame_counter.irq_disable;
                }
                4.. => unreachable!(),
            }
            self.frame_counter.step += 1;
            if self.frame_counter.step >= 4 {
                self.frame_counter.step = 0;
            }
        }
    }
    fn tick_length_counters(&mut self) {}
    fn tick_envelopes(&mut self) {}

    fn produce_sample(&mut self) {
        if self.cycles_since_sample < CYCLES_PER_SAMPLE {
            self.cycles_since_sample += 1;
            return;
        }
        self.cycles_since_sample = 0;

        let sample = self.mix();
        // This is where I'd put my audio output..
        // If I HAD ANY!!!
    }
    fn mix(&mut self) -> f32 {
        let pulse_0 = 0.0;
        let pulse_1 = 0.0;
        let triangle = 0.0;
        let noise = 0.0;
        let dmc = self.dmc.sample as f64;

        let pulse_zero = pulse_0 == 0.0 && pulse_1 == 0.0;
        let tnd_zero = triangle == 0.0 && noise == 0.0 && dmc == 0.0;

        let square_denom = 8128.0 / (pulse_0 + pulse_1) + 100.0;
        let square_out = if pulse_zero {
            0.0
        } else {
            95.88 / square_denom
        };

        let triangle = triangle / 8227.0;
        let noise = noise / 12241.0;
        let dmc = dmc / 22638.0;
        let tnd_denom = 1.0 / (triangle + noise + dmc) + 100.0;
        let tnd_out = if tnd_zero { 0.0 } else { 159.79 / tnd_denom };

        let output = square_out + tnd_out;
        let sample = ((output * 2.0) - 1.0) as f32;
        sample
    }

    fn perform_dma(&mut self, cpu: &mut CpuBus) {
        if self.dma.dmc_dma == DmcDma::ToReceive {
            let deltas = cpu.data();
            self.dmc.sample_buffer = Some(deltas);
            self.dma.dmc_dma = DmcDma::Idle;
        }

        self.dma.perform_dma(cpu);
    }
    fn update_dmc(&mut self) {
        self.update_dmc_output();
        self.update_dmc_dma();
    }
    fn update_dmc_output(&mut self) {
        if self.dmc.cycles_since_last < self.dmc.wait_cycles {
            self.dmc.cycles_since_last += 1;
            return;
        }
        self.dmc.cycles_since_last = 0;

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
            0x4014 => {
                if cpu.read() {
                    return;
                };
                self.dma.start_oam_dma(cpu.data());
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
            0x4017 => {
                if cpu.read() {
                    return;
                };
                self.frame_counter.mode = cpu.data() & 128 != 0;
                self.frame_counter.irq_disable = cpu.data() & 64 != 0;
                self.frame_counter.step = 0;
                self.frame_counter.cycles_until_step = 0;
            }
            _ => (),
        }
    }
    fn assert_irqs(&self, cpu: &mut CpuBus) {
        let irq = self.status.dmc_irq || self.status.frame_irq;
        cpu.or_irq(irq);
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
    cycles_since_last: u16,

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
            cycles_since_last: 0,

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

struct FrameCounter {
    mode: bool,
    irq_disable: bool,

    step: u8,
    cycles_until_step: u16,
}
impl FrameCounter {
    fn init() -> Self {
        Self {
            mode: false,
            irq_disable: true,
            step: 0,
            cycles_until_step: 0,
        }
    }

    const CYCLES_PER_STEP: u16 = 7457;
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
                //eprintln!("DMC read from {:x}", self.dmc_address);
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
