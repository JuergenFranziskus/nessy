use cpal::traits::DeviceTrait;
use cpal::traits::HostTrait;
use cpal::traits::StreamTrait;
use cpal::Host;
use cpal::SampleFormat;
use cpal::Stream;
use cpal::SupportedStreamConfigRange;
use cpu_6502::Cpu;
use futures::executor::block_on;
use nessy::mapper::Mapper;
use nessy::ppu::SCREEN_HEIGHT;
use nessy::ppu::SCREEN_PIXELS;
use nessy::ppu::SCREEN_WIDTH;
use nessy::{
    input::Controller,
    mapper::{get_mapper, DynMapper, MapperBus},
    nesbus::{CpuBus, NesBus},
    ppu::{Ppu, PpuBus},
    rom::Rom,
    simple_debug,
};
use parking_lot::Mutex;
use pixely::framebuffer::FrameBuffer;
use pixely::{framebuffer::Pixel, FrameBufferDesc, Pixely, PixelyDesc, WindowDesc};
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::{
    io::stdout,
    time::{Duration, Instant},
};
use wgpu::{
    Adapter, Backends, Device, DeviceDescriptor, Instance, InstanceDescriptor, PowerPreference,
    Queue, RequestAdapterOptions,
};
use winit::{
    event::{ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    let ev_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&ev_loop).unwrap();
    let (instance, adapter, device, queue) = init_wgpu();
    let mut pixely = Pixely::new(PixelyDesc {
        window: WindowDesc {
            window: &window,
            width: window.inner_size().width as usize,
            height: window.inner_size().height as usize,
        },
        buffer: FrameBufferDesc {
            width: SCREEN_WIDTH,
            height: SCREEN_HEIGHT,
        },
        instance: &instance,
        adapter: &adapter,
        device: &device,
        queue: &queue,
    })
    .unwrap();

    let (host, audio_device) = init_audio();

    let (cpu, bus, framebuffer, ctrl_inputs) = start_nes();
    let running = Arc::new(AtomicBool::new(true));
    let sound_stream = start_audio_stream(&audio_device, &bus);
    sound_stream.play().unwrap();

    let mut emu_thread = Some(start_emu_thread(cpu, bus, Arc::clone(&running)));

    let frame_duration = Duration::from_secs_f64(1.0 / 60.0);
    let mut next_frame = Instant::now();

    ev_loop.run(move |ev, _, cf| match ev {
        Event::WindowEvent { event, .. } => match event {
            WindowEvent::Resized(size) => {
                pixely.resize_surface(size.width as usize, size.height as usize);
            }
            WindowEvent::CloseRequested => {
                *cf = ControlFlow::Exit;
            }
            WindowEvent::KeyboardInput { input, .. } => handle_keyboard(&ctrl_inputs, input),
            _ => (),
        },
        Event::MainEventsCleared => {
            let buffer = framebuffer.lock();
            update_framebuffer(&buffer, pixely.buffer_mut());
            drop(buffer);

            pixely.render(&device, &queue).unwrap();

            next_frame += frame_duration;
            let now = Instant::now();
            if now < next_frame {
                spin_sleep::sleep(next_frame - now);
            }
            *cf = ControlFlow::Poll;
        }
        Event::LoopDestroyed => {
            sound_stream.pause().unwrap();
            let Some(handle) = emu_thread.take() else {
                unreachable!()
            };
            running.store(false, Ordering::Relaxed);
            handle.join().unwrap();

            // Mention the host so it gets moved into the closure and dropped properly.
            // Since the ev_loop hijacks the main thread, everything outside the closure doesn't get dropped.
            let _id = host.id();
        }
        _ => (),
    })
}

fn handle_keyboard(inputs: &[Arc<Mutex<Controller>>; 2], input: winit::event::KeyboardInput) {
    let Some(keycode) = input.virtual_keycode else {
        return;
    };
    let function = match keycode {
        VirtualKeyCode::I => Controller::set_up,
        VirtualKeyCode::K => Controller::set_down,
        VirtualKeyCode::J => Controller::set_left,
        VirtualKeyCode::L => Controller::set_right,
        VirtualKeyCode::D => Controller::set_a,
        VirtualKeyCode::F => Controller::set_b,
        VirtualKeyCode::S => Controller::set_select,
        VirtualKeyCode::Return => Controller::set_start,
        _ => return,
    };

    let state = match input.state {
        ElementState::Pressed => true,
        ElementState::Released => false,
    };

    function(&mut inputs[0].lock(), state);
}
fn update_framebuffer(ppu_buffer: &[u8; SCREEN_PIXELS], framebuffer: &mut FrameBuffer) {
    for y in 0..SCREEN_HEIGHT {
        for x in 0..SCREEN_WIDTH {
            let i = y * SCREEN_WIDTH + x;
            let color = translate_color(ppu_buffer[i]);
            framebuffer.set_pixel(x, y, color);
        }
    }
}

fn init_wgpu() -> (Instance, Adapter, Device, Queue) {
    let instance = Instance::new(InstanceDescriptor {
        backends: Backends::PRIMARY,
        dx12_shader_compiler: Default::default(),
    });

    let adapter = instance.request_adapter(&RequestAdapterOptions {
        power_preference: PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
    });
    let adapter = block_on(adapter).unwrap();

    let device = adapter.request_device(
        &DeviceDescriptor {
            label: None,
            features: adapter.features(),
            limits: adapter.limits(),
        },
        None,
    );
    let (device, queue) = block_on(device).unwrap();

    (instance, adapter, device, queue)
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
        debug,
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

const DEBUG: bool = false;
fn debug(cycle: u64, cpu: &Cpu, bus: CpuBus, ppu: &Ppu, ppu_bus: PpuBus, mapper_bus: MapperBus) {
    if !DEBUG {
        return;
    };
    simple_debug(cycle, cpu, bus, ppu, ppu_bus, mapper_bus, stdout()).unwrap();
}

fn translate_color(color: u8) -> Pixel {
    let index = color as usize * 3;
    let r = PALETTE[index + 0];
    let g = PALETTE[index + 1];
    let b = PALETTE[index + 2];

    Pixel {
        red: r,
        green: g,
        blue: b,
        alpha: 255,
    }
}

static PALETTE: &[u8] = include_bytes!("ntscpalette.pal");
