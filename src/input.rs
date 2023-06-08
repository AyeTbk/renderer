use std::collections::HashMap;

use glam::{Vec2, Vec3};
use winit::event::VirtualKeyCode;

#[derive(Debug, Default)]
pub struct Input {
    pub keymap: HashMap<VirtualKeyCode, bool>,
    pub mod_shift: bool,
    pub pointer_pos: Option<Vec2>,
    pub previous_pointer_pos: Option<Vec2>,
    //
    pub delta_view: Vec2,
    pub movement: Vec3,
    pub fast: bool,
}

impl Input {
    pub fn is_pressed(&self, key: VirtualKeyCode) -> bool {
        self.keymap.get(&key).copied().unwrap_or_default()
    }

    pub fn axis_strength(&self, positive: VirtualKeyCode, negtive: VirtualKeyCode) -> f32 {
        let positive_strength = self.is_pressed(positive) as u8 as f32;
        let negative_strength = self.is_pressed(negtive) as u8 as f32;
        positive_strength - negative_strength
    }
}
