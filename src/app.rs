use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use cpal::{
    traits::{DeviceTrait, HostTrait},
    Host, SampleFormat, Stream, SupportedStreamConfigRange,
};
use cpu_6502::Cpu;
use nes_rom_parser::Rom;
use nessy::{
    input::Controller,
    mapper::{get_mapper, DynMapper, Mapper},
    nesbus::NesBus,
    ppu::FRAMEBUFFER_PIXELS,
};
use parking_lot::Mutex;
use winit::{
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

use crate::{ENABLE_AUDIO, ROM_FILE};

pub struct App {
    pub window: Arc<Window>,
    pub _host: Host,
    pub _audio_device: cpal::Device,
    pub sound_stream: Stream,
    pub emu_thread: Option<JoinHandle<()>>,
    pub running: Arc<AtomicBool>,
    pub _framebuffer: Arc<Mutex<[u8; FRAMEBUFFER_PIXELS]>>,
    pub ctrl_inputs: [Arc<Mutex<Controller>>; 2],
}
impl App {
    pub fn init() -> (App, EventLoop<()>) {
        let ev_loop = EventLoop::new().unwrap();
        let window = Arc::new(WindowBuilder::new().build(&ev_loop).unwrap());

        let (cpu, bus, framebuffer, ctrl_inputs) = start_nes();
        let running = Arc::new(AtomicBool::new(true));

        let (host, audio_device) = init_audio();
        let sound_stream = start_audio_stream(&audio_device, &bus);

        let emu_thread = start_emu_thread(cpu, bus, running.clone());

        let app = Self {
            window,
            _host: host,
            _audio_device: audio_device,
            sound_stream,
            emu_thread: Some(emu_thread),
            running,
            _framebuffer: framebuffer,
            ctrl_inputs,
        };

        (app, ev_loop)
    }
}

fn start_nes() -> (
    Cpu,
    NesBus<DynMapper>,
    Arc<Mutex<[u8; FRAMEBUFFER_PIXELS]>>,
    [Arc<Mutex<Controller>>; 2],
) {
    let src = std::fs::read(ROM_FILE).unwrap();
    let rom = Rom::parse(&src).unwrap();
    eprintln!("{:#?}", rom.header);
    let mapper = get_mapper(&rom);

    let framebuffer = Arc::new(Mutex::new([0; FRAMEBUFFER_PIXELS]));
    let rec_ctrl_0 = Arc::new(Mutex::new(Controller(0)));
    let rec_ctrl_1 = Arc::new(Mutex::new(Controller(0)));

    let cpu = Cpu::new();
    let bus = NesBus::new(
        mapper,
        Arc::clone(&framebuffer),
        [Arc::clone(&rec_ctrl_0), Arc::clone(&rec_ctrl_1)],
    );

    (cpu, bus, framebuffer, [rec_ctrl_0, rec_ctrl_1])
}

fn init_audio() -> (Host, cpal::Device) {
    let host = cpal::default_host();
    let device = host.default_output_device().unwrap();
    (host, device)
}

fn start_audio_stream(out: &cpal::Device, bus: &NesBus<impl Mapper>) -> Stream {
    let samples = Arc::clone(bus.apu().samples());

    let configs = out.supported_output_configs().unwrap();
    let config = configs
        .max_by(SupportedStreamConfigRange::cmp_default_heuristics)
        .unwrap();
    assert_eq!(config.sample_format(), SampleFormat::F32);
    let config = config.with_max_sample_rate().config();

    eprintln!("Chose audio config: {config:#?}");

    let mut local_buffer = Vec::with_capacity(44100);

    out.build_output_stream(
        &config,
        move |data: &mut [f32], _| {
            let mut buffer = samples.lock();
            if ENABLE_AUDIO {
                local_buffer.clone_from(&buffer);
            } else {
                local_buffer.clear();
            }
            buffer.clear();
            drop(buffer);

            let data_len = data.len() as f32;
            let buffer_len = local_buffer.len() as f32;
            for (i, val) in data.into_iter().enumerate() {
                if local_buffer.is_empty() {
                    *val = 0.0;
                    continue;
                }

                let percent = i as f32 / data_len;
                let buffer_i = (percent * buffer_len) as usize;
                let sample = local_buffer[buffer_i];
                *val = sample;
            }
        },
        move |err| {
            eprintln!("Error in audio output: {err}");
        },
        None,
    )
    .unwrap()
}

fn start_emu_thread(
    mut cpu: Cpu,
    mut bus: NesBus<DynMapper>,
    running: Arc<AtomicBool>,
) -> JoinHandle<()> {
    let cycles_per_second = 1_789773;
    let cycle_time = Duration::from_secs_f64(1.0 / cycles_per_second as f64);
    let mut next_cycle = Instant::now();

    std::thread::spawn(move || {
        while running.load(Ordering::Relaxed) {
            let start_cycle = bus.cycles();
            for _ in 0..1000 {
                cpu.exec(&mut bus);
            }
            let end_cycle = bus.cycles();
            let took_cycles = end_cycle - start_cycle;

            next_cycle += cycle_time * took_cycles as u32;

            let now = Instant::now();
            if now < next_cycle {
                spin_sleep::sleep(next_cycle - now);
            }
        }
    })
}
