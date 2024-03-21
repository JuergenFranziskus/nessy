use cpal::traits::DeviceTrait;
use cpal::traits::HostTrait;
use cpal::traits::StreamTrait;
use cpal::Host;
use cpal::SampleFormat;
use cpal::Stream;
use cpal::SupportedStreamConfigRange;
use cpu_6502::Cpu;
use nes_rom_parser::Rom;
use nessy::mapper::Mapper;
use nessy::ppu::SCREEN_HEIGHT;
use nessy::ppu::SCREEN_PIXELS;
use nessy::ppu::SCREEN_WIDTH;
use nessy::{
    input::Controller,
    mapper::{get_mapper, DynMapper},
    nesbus::NesBus,
};
use parking_lot::Mutex;
use softbuffer::Buffer;
use softbuffer::Context;
use softbuffer::Surface;
use std::num::NonZeroU32;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use winit::dpi::PhysicalSize;
use winit::keyboard::KeyCode;
use winit::keyboard::PhysicalKey;
use winit::window::Window;
use winit::{
    event::{ElementState, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    let (mut app, ev_loop) = App::init();
    let frame_duration = Duration::from_secs_f64(1.0 / 60.0);
    let mut next_frame = Instant::now();

    let res = ev_loop.run(move |ev, loop_target| match ev {
        Event::WindowEvent { event, .. } => match event {
            WindowEvent::CloseRequested => {
                loop_target.exit();
            }
            WindowEvent::Resized(size) => {
                if let Err(err) = app.surface.resize(
                    NonZeroU32::new(size.width).unwrap(),
                    NonZeroU32::new(size.height).unwrap(),
                ) {
                    eprintln!(
                        "Failed to resize render-surface to {} by {}: {err}",
                        size.width, size.height
                    );
                }
            }
            WindowEvent::KeyboardInput { event, .. } => handle_keyboard(&app.ctrl_inputs, event),
            WindowEvent::RedrawRequested => {
                if let Ok(to) = app.surface.buffer_mut() {
                    let from = app.framebuffer.lock();

                    let size = app.window.inner_size();
                    update_framebuffer(&from, to, size);
                } else {
                    eprintln!("Failed to acquire render-surface for rendering");
                }

                next_frame += frame_duration;
                let now = Instant::now();
                if now < next_frame {
                    let dur = next_frame - now;
                    spin_sleep::sleep(dur);
                }
                loop_target.set_control_flow(ControlFlow::Poll);
            }
            _ => (),
        },
        Event::AboutToWait => {
            app.window.request_redraw();
        }
        Event::LoopExiting => {
            app.sound_stream.pause().unwrap();
            let Some(handle) = app.emu_thread.take() else {
                unreachable!()
            };
            app.running.store(false, Ordering::Relaxed);
            handle.join().unwrap();
        }
        _ => (),
    });

    res.unwrap();
}

fn update_framebuffer(
    from: &[u8; SCREEN_PIXELS],
    mut to: Buffer<Arc<Window>, Arc<Window>>,
    size: PhysicalSize<u32>,
) {
    let width = size.width;
    let height = size.height;

    for i in 0..(width * height) {
        let y = i / width;
        let x = i % width;

        let from_x = x * SCREEN_WIDTH as u32 / width;
        let from_y = y * SCREEN_HEIGHT as u32 / height;
        let from_i = from_y * SCREEN_WIDTH as u32 + from_x;

        let pixel = from[from_i as usize];
        let [r, g, b] = translate_color(pixel);
        to[i as usize] = b | (g << 8) | (r << 16);
    }

    if let Err(err) = to.present() {
        eprintln!("Failed to present render-surface: {err}");
    }
}

struct App {
    window: Arc<Window>,
    host: Host,
    audio_device: cpal::Device,
    sound_stream: Stream,

    context: Context<Arc<Window>>,
    surface: Surface<Arc<Window>, Arc<Window>>,

    emu_thread: Option<JoinHandle<()>>,
    running: Arc<AtomicBool>,
    framebuffer: Arc<Mutex<[u8; SCREEN_PIXELS]>>,
    ctrl_inputs: [Arc<Mutex<Controller>>; 2],
}
impl App {
    fn init() -> (App, EventLoop<()>) {
        let ev_loop = EventLoop::new().unwrap();
        let window = Arc::new(WindowBuilder::new().build(&ev_loop).unwrap());

        let context = Context::new(window.clone()).unwrap();
        let surface = Surface::new(&context, window.clone()).unwrap();

        let (cpu, bus, framebuffer, ctrl_inputs) = start_nes();
        let running = Arc::new(AtomicBool::new(true));

        let (host, audio_device) = init_audio();
        let sound_stream = start_audio_stream(&audio_device, &bus);

        let emu_thread = start_emu_thread(cpu, bus, running.clone());

        let app = Self {
            window,
            host,
            audio_device,
            sound_stream,
            context,
            surface,
            emu_thread: Some(emu_thread),
            running,
            framebuffer,
            ctrl_inputs,
        };

        (app, ev_loop)
    }
}

fn handle_keyboard(inputs: &[Arc<Mutex<Controller>>; 2], input: winit::event::KeyEvent) {
    let keycode = input.physical_key;
    let function = match keycode {
        PhysicalKey::Code(KeyCode::KeyI) => Controller::set_up,
        PhysicalKey::Code(KeyCode::KeyK) => Controller::set_down,
        PhysicalKey::Code(KeyCode::KeyJ) => Controller::set_left,
        PhysicalKey::Code(KeyCode::KeyL) => Controller::set_right,
        PhysicalKey::Code(KeyCode::KeyD) => Controller::set_a,
        PhysicalKey::Code(KeyCode::KeyF) => Controller::set_b,
        PhysicalKey::Code(KeyCode::KeyS) => Controller::set_select,
        PhysicalKey::Code(KeyCode::Enter) => Controller::set_start,
        _ => return,
    };

    let state = match input.state {
        ElementState::Pressed => true,
        ElementState::Released => false,
    };

    function(&mut inputs[0].lock(), state);
}

fn init_audio() -> (Host, cpal::Device) {
    let host = cpal::default_host();
    let device = host.default_output_device().unwrap();
    (host, device)
}

fn start_nes() -> (
    Cpu,
    NesBus<DynMapper>,
    Arc<Mutex<[u8; SCREEN_PIXELS]>>,
    [Arc<Mutex<Controller>>; 2],
) {
    let src = std::fs::read("./roms/SuperMarioBros.nes").unwrap();
    let rom = Rom::parse(&src).unwrap();
    eprintln!("{:#?}", rom.header);
    let mapper = get_mapper(&rom);

    let framebuffer = Arc::new(Mutex::new([0; SCREEN_PIXELS]));
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
            local_buffer.clone_from(&buffer);
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
    let cycles_per_second = 1_789773.0;
    let cycle_time = Duration::from_secs_f64(1.0 / cycles_per_second);
    let mut next_cycle = Instant::now();

    std::thread::spawn(move || {
        while running.load(Ordering::Relaxed) {
            if Instant::now() < next_cycle {
                continue;
            };
            let start_cycle = bus.cycles();
            cpu.exec(&mut bus);
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

fn translate_color(color: u8) -> [u32; 3] {
    let index = color as usize * 3;
    let r = PALETTE[index + 0];
    let g = PALETTE[index + 1];
    let b = PALETTE[index + 2];

    [r as u32, g as u32, b as u32]
}

static PALETTE: &[u8] = include_bytes!("ntscpalette.pal");
