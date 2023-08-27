use cpu_6502::Bus;
use cpu_6502::Cpu;
use futures::executor::block_on;
use nessy::{
    input::Controller,
    mapper::{get_mapper, DynMapper, MapperBus},
    nesbus::{CpuBus, NesBus},
    ppu::{Ppu, PpuBus},
    rom::Rom,
    simple_debug,
};
use pixely::{framebuffer::Pixel, FrameBufferDesc, Pixely, PixelyDesc, WindowDesc};
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
            width: 256,
            height: 240,
        },
        instance: &instance,
        adapter: &adapter,
        device: &device,
        queue: &queue,
    })
    .unwrap();

    let (mut cpu, mut bus) = start_nes();
    let mut last_nmi = false;

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
            WindowEvent::KeyboardInput { input, .. } => handle_keyboard(&mut bus, input),
            _ => (),
        },
        Event::MainEventsCleared => {
            loop {
                cpu.exec(&mut bus);
                let nmi = bus.nmi();
                let quit = nmi && !last_nmi;
                last_nmi = nmi;
                if quit {
                    break;
                };
            }

            let ppu_buffer = bus.ppu().framebuffer();
            let framebuffer = pixely.buffer_mut();
            for y in 0..240 {
                for x in 0..256 {
                    let index = y * 256 + x;
                    let color = ppu_buffer[index];
                    let pixel = translate_color(color);
                    framebuffer.set_pixel(x, y, pixel);
                }
            }
            pixely.render(&device, &queue).unwrap();

            next_frame += frame_duration;
            let now = Instant::now();
            if next_frame > now {
                let sleep = next_frame - now;
                std::thread::sleep(sleep);
            }
            *cf = ControlFlow::Poll;
        }
        _ => (),
    })
}

fn handle_keyboard<O>(bus: &mut NesBus<O>, input: winit::event::KeyboardInput) {
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

    let input = bus.input_mut();
    let controller = input.controller_mut(0);
    function(controller, state);
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

fn start_nes() -> (Cpu, NesBus<DynMapper>) {
    let src = std::fs::read("./roms/DoubleDribble.nes").unwrap();
    let rom = Rom::parse(&src).unwrap();
    eprintln!("{:#?}", rom.header);
    let mapper = get_mapper(&rom);

    let cpu = Cpu::new();
    let bus = NesBus::new(mapper, debug);

    (cpu, bus)
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
