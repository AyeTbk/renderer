use std::sync::Arc;

use asset_image::Image;
use glam::{Affine3A, Mat3A, Quat, UVec2, Vec2, Vec3, Vec3A};
use renderer::{Color, Engine, Light, Node, ToneMapping};
use winit::{
    dpi::PhysicalSize,
    event::{DeviceEvent, ElementState, Event, KeyEvent, MouseButton, WindowEvent},
    event_loop::EventLoop,
    keyboard::{Key, KeyCode, NamedKey, PhysicalKey},
    window::WindowBuilder,
};

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("renderer")
            .build(&event_loop)
            .unwrap(),
    );

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
        // .load_scene("data/scenes/the-sphere.glb")
        // .load_scene("data/scenes/tri.glb")
        // .load_scene("data/scenes/uvs.glb")
        // .load_scene("data/scenes/checker-world.glb")
        .unwrap();
    eng.scene = eng.asset_server.get(scene).clone();

    // Make ui
    make_ui(&mut eng.scene);

    //= Load extra subscene =
    let helmet = eng
        .asset_server
        .load_scene("data/scenes/flight/FlightHelmet.gltf")
        .unwrap();
    let helmet_scene = eng.asset_server.get(helmet).clone();
    eng.scene.add_child(
        eng.scene.root,
        Node::new_scene(helmet_scene)
            .with_transform(Affine3A::from_rotation_y(-std::f32::consts::FRAC_PI_2)),
    );

    // Setup first person camera
    eng.scene.add_child(
        eng.scene.root,
        Node::new_camera(Default::default())
            .with_transform(
                Affine3A::from_translation(Vec3::new(3.0, 2.0, -3.0))
                    * Affine3A::from_rotation_y(-0.8),
            )
            .with_update(|this, ctx| {
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

    // Lights
    let dirlight = eng.scene.add_child(
        eng.scene.root,
        Node::new_light(Light::directional().with_color(Color::new(1.0, 0.9, 0.8, 3.5)))
            .with_transform(
                Affine3A::look_to_lh(
                    Vec3::new(-1.5, 20.0, -6.0),
                    Vec3::new(0.05, -1.0, 0.2),
                    Vec3::Y,
                )
                .inverse(),
            )
            .with_update(|node, ctx| {
                let angle = ctx.time.delta * 0.025;
                node.transform = Affine3A::from_rotation_y(angle) * node.transform;
            }),
    );
    let dirlight = eng.scene.make_unique_node_id(dirlight);

    // = Point light =
    eng.scene.add_child(
        eng.scene.root,
        Node::new_light(Light::point(4.0).with_color(Color::new(1.0, 0.01, 0.005, 2.0)))
            .with_transform(Affine3A::from_translation(Vec3::new(0.0, 1.0, 1.0))),
    );

    event_loop
        .run(move |event, elwt| {
            match event {
                Event::WindowEvent {
                    ref event,
                    window_id,
                } if window_id == window.id() => match event {
                    WindowEvent::CloseRequested
                    | WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                state: ElementState::Pressed,
                                logical_key: Key::Named(NamedKey::Escape),
                                ..
                            },
                        ..
                    } => elwt.exit(),
                    WindowEvent::Resized(physical_size) => {
                        eng.set_window_inner_size(physical_size_to_uvec2(*physical_size));
                    }
                    // WindowEvent::ScaleFactorChanged { .. } => {
                    //     // eng.set_window_inner_size(physical_size_to_uvec2(**new_inner_size));
                    // }

                    // Input stuff
                    WindowEvent::ModifiersChanged(modifiers) => {
                        eng.input.mod_shift = modifiers.state().shift_key();
                    }
                    WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                state,
                                physical_key: PhysicalKey::Code(keycode),
                                ..
                            },
                        ..
                    } => {
                        eng.input
                            .keymap
                            .insert(*keycode, *state == ElementState::Pressed);

                        if *state == ElementState::Pressed {
                            if *keycode == KeyCode::KeyF {
                                let fullscreen_mode = if window.fullscreen().is_none() {
                                    Some(winit::window::Fullscreen::Borderless(None))
                                } else {
                                    None
                                };
                                window.set_fullscreen(fullscreen_mode);
                            }

                            if *keycode == KeyCode::KeyM {
                                eng.visual_server.set_msaa(4);
                            } else if *keycode == KeyCode::KeyN {
                                eng.visual_server.set_msaa(1);
                            }

                            if *keycode == KeyCode::KeyP {
                                eng.visual_server.set_render_size_factor(4.0);
                            } else if *keycode == KeyCode::KeyO {
                                eng.visual_server.set_render_size_factor(2.0);
                            } else if *keycode == KeyCode::KeyI {
                                eng.visual_server.set_render_size_factor(1.0);
                            } else if *keycode == KeyCode::KeyU {
                                eng.visual_server.set_render_size_factor(0.5);
                            } else if *keycode == KeyCode::KeyY {
                                eng.visual_server.set_render_size_factor(0.25);
                            }

                            if *keycode == KeyCode::KeyH {
                                eng.visual_server.unset_fullscreen_texture();
                            } else if *keycode == KeyCode::KeyJ {
                                eng.visual_server.set_depth_fullscreen_texture();
                            } else if *keycode == KeyCode::KeyK {
                                eng.visual_server
                                    .set_shadow_map_fullscreen_texture(dirlight);
                            }
                        }
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        let pointer_pos = Vec2::new(position.x as f32, position.y as f32);
                        eng.input.pointer_pos = pointer_pos;
                    }
                    WindowEvent::MouseInput { button, state, .. } => {
                        eng.input
                            .buttonmap
                            .insert(*button, *state == ElementState::Pressed);

                        if *button == MouseButton::Right {
                            if *state == ElementState::Pressed {
                                if eng.input.pointer_grabbed {
                                    eng.input.pointer_grabbed = false;
                                    window
                                        .set_cursor_grab(winit::window::CursorGrabMode::None)
                                        .unwrap();
                                    window.set_cursor_visible(true);
                                } else {
                                    eng.input.pointer_grabbed = true;
                                    window
                                        .set_cursor_grab(winit::window::CursorGrabMode::Confined)
                                        .unwrap();
                                    window.set_cursor_visible(false);
                                }
                            }
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        eng.update();
                        match eng.visual_server.render() {
                            Ok(_) => {}
                            Err(wgpu::SurfaceError::Lost) => eng
                                .visual_server
                                .set_render_size(eng.visual_server.render_size()),
                            Err(wgpu::SurfaceError::OutOfMemory) => {
                                elwt.exit();
                            }
                            Err(e) => {
                                dbg!(e);
                            }
                        }
                    }
                    _ => {}
                },
                Event::DeviceEvent {
                    event: DeviceEvent::MouseMotion { delta },
                    ..
                } => {
                    if eng.input.pointer_grabbed {
                        eng.input.pointer_delta += Vec2::new(delta.0 as f32, delta.1 as f32);
                    }
                }
                Event::AboutToWait => {
                    window.request_redraw();
                }
                _ => {}
            }
        })
        .unwrap();
}

fn physical_size_to_uvec2(size: PhysicalSize<u32>) -> UVec2 {
    UVec2::new(size.width, size.height)
}

fn make_ui(scene: &mut renderer::Scene) {
    use renderer::ui::helpers::*;
    use renderer::ui::*;

    UiBuilder::new(scene).container(
        Node::new_uibox(UiBox {
            layout: Layout {
                width: 225.0,
                v_extend: true,
                padding: 20.0,
                ..Default::default()
            },
            style: Style {
                color: Color::new(0.13, 0.13, 0.15, 0.85),
                ..Default::default()
            },
            ..Default::default()
        })
        .with_update(|node, ctx| {
            if ctx.input.is_just_pressed(KeyCode::Tab) {
                let uibox = node.as_uibox_mut().unwrap();
                uibox.hide = !uibox.hide;
            }
        }),
        |b| {
            b //
                .note("press TAB to toggle")
                .title("Antialiasing")
                .button_group(|b| {
                    b.button(
                        "None",
                        Some(|ctx| ctx.visual_server.set_msaa(1)),
                        Some(|node, ctx| {
                            node.as_uibox_mut().unwrap().active =
                                ctx.visual_server.msaa_sample_count() == 1;
                        }),
                    )
                    .button(
                        "MSAAx4",
                        Some(|ctx| ctx.visual_server.set_msaa(4)),
                        Some(|node, ctx| {
                            node.as_uibox_mut().unwrap().active =
                                ctx.visual_server.msaa_sample_count() == 4;
                        }),
                    );
                })
                .title("Resolution factor")
                .button_group(|b| {
                    b.button(
                        ".25x",
                        Some(|ctx| ctx.visual_server.set_render_size_factor(0.25)),
                        Some(|node, ctx| {
                            node.as_uibox_mut().unwrap().active =
                                ctx.visual_server.render_size_factor() == 0.25;
                        }),
                    )
                    .button(
                        ".5x",
                        Some(|ctx| ctx.visual_server.set_render_size_factor(0.5)),
                        Some(|node, ctx| {
                            node.as_uibox_mut().unwrap().active =
                                ctx.visual_server.render_size_factor() == 0.5;
                        }),
                    )
                    .button(
                        "1x",
                        Some(|ctx| ctx.visual_server.set_render_size_factor(1.0)),
                        Some(|node, ctx| {
                            node.as_uibox_mut().unwrap().active =
                                ctx.visual_server.render_size_factor() == 1.0;
                        }),
                    )
                    .button(
                        "2x",
                        Some(|ctx| ctx.visual_server.set_render_size_factor(2.0)),
                        Some(|node, ctx| {
                            node.as_uibox_mut().unwrap().active =
                                ctx.visual_server.render_size_factor() == 2.0;
                        }),
                    );
                })
                .title("Tone mapping")
                .button_list(|b| {
                    b.button(
                        "None",
                        Some(|ctx| ctx.visual_server.set_tone_mapping(ToneMapping::None)),
                        Some(|node, ctx| {
                            node.as_uibox_mut().unwrap().active =
                                ctx.visual_server.tone_mapping() == ToneMapping::None;
                        }),
                    )
                    .button(
                        "Reinhard",
                        Some(|ctx| ctx.visual_server.set_tone_mapping(ToneMapping::Reinhard)),
                        Some(|node, ctx| {
                            node.as_uibox_mut().unwrap().active =
                                ctx.visual_server.tone_mapping() == ToneMapping::Reinhard;
                        }),
                    );
                });
        },
    );
}
