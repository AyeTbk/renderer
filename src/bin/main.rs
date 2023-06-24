use asset_image::Image;
use glam::{Affine3A, Mat3A, Quat, UVec2, Vec2, Vec3, Vec3A};
use renderer::{Engine, Node};
use winit::{
    dpi::PhysicalSize,
    event::{DeviceEvent, ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("renderer")
        .build(&event_loop)
        .unwrap();

    let mut cursor_grabbed = false;

    let mut eng = Engine::new(&window);

    // Load font
    let font_handle = eng.asset_server.load::<Image>("data/sdffont.png");
    eng.visual_server
        .set_font_image(font_handle, &eng.asset_server);

    // Load scene
    let scene = eng
        .asset_server
        .load_scene("data/scenes/sponza/Sponza.gltf")
        // .load_scene("data/scenes/flight/FlightHelmet.gltf")
        // .load_scene("data/scenes/suzanne/suzanne.gltf")
        // .load_scene("data/scenes/tri.glb")
        // .load_scene("data/scenes/uvs.glb")
        // .load_scene("data/scenes/checker-world.glb")
        .unwrap();
    eng.scene = eng.asset_server.get(scene).clone();

    // Setup first person camera
    eng.scene.add_allocate_child(
        eng.scene.root,
        Node::new_camera(Default::default())
            .with_transform(
                Affine3A::from_rotation_y(-std::f32::consts::FRAC_PI_2)
                    * Affine3A::from_translation(Vec3::new(0.0, 1.0, 0.0)),
            )
            .with_update(|this, _, ctx| {
                // Mouse look
                let look_speed = Vec2::new(6.0, 6.0);
                let delta_yaw = ctx.input.delta_view.x * look_speed.x;
                this.transform.matrix3 = Mat3A::from_rotation_y(delta_yaw) * this.transform.matrix3;

                let (_, rot, _) = this.transform.to_scale_rotation_translation();
                let (_, cur_pitch, _) = rot.to_euler(glam::EulerRot::YXZ);
                let delta_pitch = ctx.input.delta_view.y * look_speed.y;
                let target_pitch = cur_pitch + delta_pitch;
                let correct_pitch = target_pitch.clamp(-1.55, 1.55);
                let correct_delta_pitch = correct_pitch - cur_pitch;
                let pitch_rot = Mat3A::from_quat(Quat::from_axis_angle(
                    this.transform.x_axis.into(),
                    correct_delta_pitch,
                ));
                this.transform.matrix3 = pitch_rot * this.transform.matrix3;

                // WASD move
                let speed = if ctx.input.fast { 5.0 } else { 1.5 };
                let linvel = ctx.input.movement * speed * ctx.time.delta;
                let movement = this.transform.matrix3 * linvel;
                this.transform.translation += Vec3A::from(movement);
            }),
    );

    event_loop.run(move |event, _, control_flow| {
        match event {
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
                    eng.set_window_inner_size(physical_size_to_uvec2(*physical_size));
                }
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    eng.set_window_inner_size(physical_size_to_uvec2(**new_inner_size));
                }

                // Input stuff
                WindowEvent::ModifiersChanged(modifiers) => {
                    eng.input.mod_shift = modifiers.shift();
                }
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            state,
                            virtual_keycode: Some(key),
                            ..
                        },
                    ..
                } => {
                    eng.input
                        .keymap
                        .insert(*key, *state == ElementState::Pressed);

                    if *state == ElementState::Pressed {
                        if *key == VirtualKeyCode::F {
                            let fullscreen_mode = if window.fullscreen().is_none() {
                                Some(winit::window::Fullscreen::Borderless(None))
                            } else {
                                None
                            };
                            window.set_fullscreen(fullscreen_mode);
                        }

                        if *key == VirtualKeyCode::M {
                            eng.visual_server.set_msaa(4);
                        } else if *key == VirtualKeyCode::N {
                            eng.visual_server.set_msaa(1);
                        }

                        if *key == VirtualKeyCode::P {
                            eng.visual_server.set_render_size_factor(4.0);
                        } else if *key == VirtualKeyCode::O {
                            eng.visual_server.set_render_size_factor(2.0);
                        } else if *key == VirtualKeyCode::I {
                            eng.visual_server.set_render_size_factor(1.0);
                        } else if *key == VirtualKeyCode::U {
                            eng.visual_server.set_render_size_factor(0.5);
                        } else if *key == VirtualKeyCode::Y {
                            eng.visual_server.set_render_size_factor(0.25);
                        }
                    }
                }
                WindowEvent::CursorMoved { position, .. } => {
                    let pointer_pos = Vec2::new(position.x as f32, position.y as f32);
                    eng.input.pointer_pos = pointer_pos;
                }
                WindowEvent::MouseInput { state, .. } => {
                    if *state == ElementState::Pressed {
                        if cursor_grabbed {
                            cursor_grabbed = false;
                            window
                                .set_cursor_grab(winit::window::CursorGrabMode::None)
                                .unwrap();
                            window.set_cursor_visible(true);
                        } else {
                            cursor_grabbed = true;
                            window
                                .set_cursor_grab(winit::window::CursorGrabMode::Confined)
                                .unwrap();
                            window.set_cursor_visible(false);
                        }
                    }
                }
                _ => {}
            },
            Event::DeviceEvent {
                event: DeviceEvent::MouseMotion { delta },
                ..
            } => {
                if cursor_grabbed {
                    eng.input.pointer_delta += Vec2::new(delta.0 as f32, delta.1 as f32);
                }
            }
            Event::RedrawRequested(window_id) if window_id == window.id() => {
                eng.update();
                match eng.visual_server.render() {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => eng
                        .visual_server
                        .set_render_size(eng.visual_server.render_size()),
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
        }
    });
}

fn physical_size_to_uvec2(size: PhysicalSize<u32>) -> UVec2 {
    UVec2::new(size.width, size.height)
}
