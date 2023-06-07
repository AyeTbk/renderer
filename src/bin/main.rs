use glam::UVec2;
use renderer::{AssetServer, Renderer, VisualServer};
use winit::{
    dpi::PhysicalSize,
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    let mut ass = AssetServer::new();
    let scene = ass.load_scene("data/tri.glb").unwrap();

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    let mut vis = VisualServer::new(&window);
    vis.set_scene(scene, &ass);

    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == window.id() => match event {
            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state: ElementState::Pressed,
                        virtual_keycode: Some(VirtualKeyCode::Escape),
                        ..
                    },
                ..
            } => *control_flow = ControlFlow::Exit,
            WindowEvent::Resized(physical_size) => {
                vis.set_render_size(physical_size_to_uvec2(*physical_size));
            }
            WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                vis.set_render_size(physical_size_to_uvec2(**new_inner_size));
            }
            WindowEvent::CursorMoved { .. } => {
                //
            }
            _ => {}
        },
        Event::RedrawRequested(window_id) if window_id == window.id() => {
            update(&mut vis);
            match vis.render() {
                Ok(_) => {}
                Err(wgpu::SurfaceError::Lost) => vis.set_render_size(vis.render_size()),
                Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                Err(e) => {
                    dbg!(e);
                }
            }
        }
        Event::MainEventsCleared => {
            window.request_redraw();
        }
        _ => {}
    });
}

fn physical_size_to_uvec2(size: PhysicalSize<u32>) -> UVec2 {
    UVec2::new(size.width, size.height)
}

fn update(_vis: &mut VisualServer) {
    // let t = std::time::UNIX_EPOCH
    //     .elapsed()
    //     .unwrap()
    //     .as_secs_f64()
    //     .rem_euclid(24.0 * 60.0 * 60.0) as f32;
    // ren.set_clear_color(Color::new_rgb(
    //     (t.sin() + 1.0) / 24.0,
    //     ((t / 7.0).cos() + 1.0) / 24.0,
    //     ((t / 3.0).sin() + 1.0) / 24.0,
    // ))
}
