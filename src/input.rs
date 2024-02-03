use std::collections::HashMap;

use glam::{Vec2, Vec3};
use winit::{event::MouseButton, keyboard::KeyCode};

#[derive(Debug, Default)]
pub struct Input {
    pub keymap: HashMap<KeyCode, bool>,
    pub previous_keymap: HashMap<KeyCode, bool>,
    pub buttonmap: HashMap<MouseButton, bool>,
    pub mod_shift: bool,
    pub pointer_pos: Vec2,
    pub pointer_delta: Vec2,
    pub pointer_grabbed: bool,
    //
    pub delta_view: Vec2,
    pub movement: Vec3,
    pub fast: bool,
}

impl Input {
    pub fn is_pressed(&self, key: KeyCode) -> bool {
        self.keymap.get(&key).copied().unwrap_or_default()
    }

    pub fn is_just_pressed(&self, key: KeyCode) -> bool {
        let was_pressed = self.previous_keymap.get(&key).copied().unwrap_or_default();
        if !was_pressed {
            self.is_pressed(key)
        } else {
            false
        }
    }

    pub fn is_button_pressed(&self, button: MouseButton) -> bool {
        self.buttonmap.get(&button).copied().unwrap_or_default()
    }

    pub fn axis_strength(&self, positive: KeyCode, negtive: KeyCode) -> f32 {
        let positive_strength = self.is_pressed(positive) as u8 as f32;
        let negative_strength = self.is_pressed(negtive) as u8 as f32;
        positive_strength - negative_strength
    }

    pub fn swap_maps(&mut self) {
        self.previous_keymap.clear();
        self.previous_keymap.extend(self.keymap.iter());
    }
}
