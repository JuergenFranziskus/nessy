use app::App;
use nessy::input::Controller;
use renderer::Renderer;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use winit::{
    event::{ElementState, Event, WindowEvent},
    event_loop::ControlFlow,
    keyboard::{KeyCode, PhysicalKey},
};

const ROM_FILE: &str = "roms/SuperMarioBros.nes";

mod app;
mod renderer;

fn main() {
    env_logger::init();

    let (mut app, ev_loop) = App::init();
    let window = Arc::clone(&app.window);
    let mut renderer = Renderer::init(Arc::clone(&window));

    let nes_frame_time = Duration::from_secs_f64(1.0 / 60.0);
    let mut last_nes_frame = Instant::now();

    let res = ev_loop.run(move |ev, loop_target| match ev {
        Event::WindowEvent { event, .. } => {
            renderer.window_event(&event);
            match event {
                WindowEvent::CloseRequested => {
                    loop_target.exit();
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    handle_keyboard(app.nesbus.controllers_mut(), event)
                }
                WindowEvent::RedrawRequested => {
                    for _ in 0..5 {
                        if last_nes_frame.elapsed() < nes_frame_time {
                            break;
                        };
                        last_nes_frame += nes_frame_time;
                        app.run_nes_until_vsync();
                    }

                    let pixels = app.nesbus.ppu().pixels();
                    renderer.upload_pixels(pixels);
                    renderer.render();
                    loop_target.set_control_flow(ControlFlow::Poll);
                }
                _ => (),
            }
        }
        Event::AboutToWait => {
            app.window.request_redraw();
        }
        _ => (),
    });

    res.unwrap();
}

fn handle_keyboard(inputs: &mut [Controller; 2], input: winit::event::KeyEvent) {
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

    function(&mut inputs[0], state);
}
