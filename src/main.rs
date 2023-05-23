use futures::executor::block_on;
use nessy::{
    cpu::Cpu,
    mapper::{nrom::NRom, Mapper},
    nes::Nes,
    processor::InPins as RPins,
    processor::OutPins,
    rom::Rom,
};
use parking_lot::Mutex;
use pixely::{framebuffer::Pixel, FrameBufferDesc, Pixely, PixelyDesc, WindowDesc};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use wgpu::{
    Adapter, Backends, Device, DeviceDescriptor, Features, Instance, InstanceDescriptor, Limits,
    PowerPreference, Queue, RequestAdapterOptions,
};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::run_return::EventLoopExtRunReturn,
    window::{Window, WindowBuilder},
};

static CYCLES_PER_SECOND: f64 = 21_477272.0;
static CYCLES_PER_FRAME: f64 = CYCLES_PER_SECOND / 60.0;

fn main() {
    let nes = start_console();
    let nes = Arc::new(Mutex::new(nes));
    let nes_clone = Arc::clone(&nes);
    let thread_running = Arc::new(AtomicBool::new(true));
    let thread_running_clone = Arc::clone(&thread_running);

    let run_thread = std::thread::spawn(move || {
        let cycles = CYCLES_PER_FRAME.floor() as usize;
        let frame_time = Duration::from_secs_f64(1.0 / 60.0);

        loop {
            if !thread_running.load(Ordering::SeqCst) {
                break;
            }

            let start = Instant::now();
            let mut nes = nes.lock();
            for _ in 0..cycles {
                nes.master_cycle();
            }
            drop(nes);

            let frame_took = start.elapsed();
            if frame_time >= frame_took {
                let to_sleep = frame_time - frame_took;
                spin_sleep::sleep(to_sleep);
            }
        }
    });

    let nes = nes_clone;
    let thread_running = thread_running_clone;

    let mut ev_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&ev_loop).unwrap();

    let gpu = GPU::init();
    let mut pixels = init_framebuffer(&gpu, &window);
    let mut running = true;

    ev_loop.run_return(|ev, _, cf| {
        match ev {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => running = false,
                WindowEvent::Resized(size) => {
                    pixels.resize_surface(size.width as usize, size.height as usize);
                }
                _ => (),
            },
            Event::MainEventsCleared => {
                let pixel_buffer = pixels.buffer_mut();
                let nes = nes.lock();
                let framebuffer = nes.ppu().framebuffer();
                for x in 0..256 {
                    for y in 0..240 {
                        let i = x + y * 256;
                        let [red, green, blue] = framebuffer[i];
                        let pixel = Pixel {
                            red,
                            green,
                            blue,
                            alpha: 255,
                        };
                        pixel_buffer.set_pixel(x, y, pixel);
                    }
                }
                drop(nes);
                pixels.render(&gpu.device, &gpu.queue).unwrap();
            }
            _ => (),
        }

        if running {
            *cf = ControlFlow::Poll;
        } else {
            *cf = ControlFlow::Exit;
        }
    });

    thread_running.store(false, Ordering::SeqCst);
    run_thread.join().unwrap();
}

fn init_framebuffer(gpu: &GPU, window: &Window) -> Pixely {
    let size = window.inner_size();

    Pixely::new(PixelyDesc {
        window: WindowDesc {
            window,
            width: size.width as usize,
            height: size.height as usize,
        },
        buffer: FrameBufferDesc {
            width: 256,
            height: 240,
        },
        instance: &gpu.instance,
        adapter: &gpu.adapter,
        device: &gpu.device,
        queue: &gpu.queue,
    })
    .unwrap()
}

fn start_console() -> Nes<impl Mapper> {
    let rom_src = std::fs::read("roms/DonkeyKong.nes").unwrap();
    let rom = Rom::parse(&rom_src);
    eprintln!("{:#?}", rom.header);

    assert_eq!(rom.header.mapper(), 0);
    let mapper = NRom::new(rom.prg_rom, rom.chr_rom, rom.header.mirroring);
    let nes = Nes::new(mapper);
    nes
}

#[allow(dead_code)]
fn print_cycle_debug(cycle: isize, pins: RPins, out: OutPins, cpu: &Cpu, print_instruction: bool) {
    let data = if out.read { pins.data } else { out.data };
    let address = out.address;
    let rw = if out.read { "     " } else { "WRITE" };
    let sync = if out.sync { "SYNC" } else { "    " };
    let nmi = if pins.nmi { "NMI" } else { "   " };
    let irq = if pins.irq { "IRQ" } else { "   " };
    let reset = if pins.reset { "RST" } else { "   " };

    let a = cpu.a();
    let x = cpu.x();
    let y = cpu.y();
    let sp = cpu.sp();
    let pc = cpu.pc();

    let flags = cpu.flags();
    let c = if flags.carry { "C" } else { " " };
    let z = if flags.zero { "Z" } else { " " };
    let i = if flags.irq_disable { "I" } else { " " };
    let d = if flags.decimal { "D" } else { " " };
    let v = if flags.overflow { "V" } else { " " };
    let n = if flags.negative { "N" } else { " " };

    let instr = if print_instruction {
        format!("{:?} {}", cpu.opcode(), cpu.address_mode())
    } else {
        "".to_string()
    };

    println!(
        "{cycle:0>4}: {nmi} {irq} {reset} {rw} {sync} {address:0>4x} = {data:>2x}; \
        {instr:<14}     \
        A = {a:>2x}, X = {x:>2x}, Y = {y:>2x}, SP = {sp:>2x}, PC = {pc:>4x};  \
        {n}{v}  {d}{i}{z}{c}"
    );
}

#[allow(dead_code)]
fn print_vram_debug(vram: &[u8]) {
    eprintln!("VRAM:");
    let chunks = vram.chunks(16);

    for chunk in chunks {
        for &byte in chunk {
            eprint!("{byte:0>2x} ");
        }
        eprintln!();
    }
}

struct GPU {
    instance: Instance,
    adapter: Adapter,
    device: Device,
    queue: Queue,
}
impl GPU {
    fn init() -> Self {
        let instance = Instance::new(InstanceDescriptor {
            backends: Backends::PRIMARY,
            dx12_shader_compiler: Default::default(),
        });

        let adapter = instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        });
        let adapter = block_on(adapter).expect("Could not acquire graphics adapter");

        let device = adapter.request_device(
            &DeviceDescriptor {
                label: None,
                features: Features::default(),
                limits: Limits::default(),
            },
            None,
        );
        let (device, queue) = block_on(device).unwrap();

        Self {
            instance,
            adapter,
            device,
            queue,
        }
    }
}
