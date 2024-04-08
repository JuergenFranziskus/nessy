use app::App;
use cpal::traits::StreamTrait;

use nessy::input::Controller;
use parking_lot::Mutex;
use renderer::Renderer;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use winit::{
    event::{ElementState, Event, WindowEvent},
    event_loop::ControlFlow,
    keyboard::{KeyCode, PhysicalKey},
};

const ENABLE_AUDIO: bool = false;
const ROM_FILE: &str = "roms/SuperMarioBros.nes";

mod app;
mod renderer;

fn main() {
    env_logger::init();

    let (mut app, ev_loop) = App::init();
    let window = Arc::clone(&app.window);
    let mut renderer = Renderer::init(Arc::clone(&window));

    let res = ev_loop.run(move |ev, loop_target| match ev {
        Event::WindowEvent { event, .. } => {
            renderer.window_event(&event);
            match event {
                WindowEvent::CloseRequested => {
                    loop_target.exit();
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    handle_keyboard(&app.ctrl_inputs, event)
                }
                WindowEvent::RedrawRequested => {
                    renderer.render();
                    loop_target.set_control_flow(ControlFlow::Poll);
                }
                _ => (),
            }
        }
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

fn translate_color(color: u8) -> [u32; 3] {
    let index = color as usize * 3;
    let r = PALETTE[index + 0];
    let g = PALETTE[index + 1];
    let b = PALETTE[index + 2];

    [r as u32, g as u32, b as u32]
}

static PALETTE: &[u8] = include_bytes!("ntscpalette.pal");
