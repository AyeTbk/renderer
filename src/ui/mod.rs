use std::collections::BTreeMap;

use glam::Vec2;
use winit::event::MouseButton;

use crate::{
    engine::Context, renderer::pipeline2d::uibox_instance::UiBoxInstance, scene::NodeId, Color,
    Scene,
};

pub mod helpers;

#[derive(Debug, Default, Clone)]
pub struct UiBox {
    pub rect: Rect, // Determined by layout
    pub state: UiBoxState,
    pub layout: Layout,
    pub style: Style,
    pub text: Option<String>,
    pub on_click: Option<fn(&mut Context)>,
    pub active: bool,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Rect {
    pub pos: Vec2, // Top left
    pub size: Vec2,
}

impl Rect {
    pub fn contains(&self, point: Vec2) -> bool {
        (self.pos.x <= point.x && point.x <= self.pos.x + self.size.x)
            && (self.pos.y <= point.y && point.y <= self.pos.y + self.size.y)
    }

    pub fn shrunk(&self, amount: f32) -> Self {
        let half_amount = amount / 2.0;
        Self {
            pos: self.pos + Vec2::splat(half_amount),
            size: self.size - Vec2::splat(amount),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum UiBoxState {
    #[default]
    Normal,
    Hovered,
    Pressed,
}

#[derive(Debug, Default, Clone)]
pub struct Layout {
    pub h_extend: bool,
    pub v_extend: bool,
    pub width: f32,
    pub height: f32,
    pub padding: f32,
    pub direction: LayoutDirection,
    pub gap: f32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum LayoutDirection {
    #[default]
    Vertical,
    Horizontal,
}

impl LayoutDirection {
    fn axis_select<T>(&self, h_value: T, v_value: T) -> T {
        match self {
            Self::Horizontal => h_value,
            Self::Vertical => v_value,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Style {
    pub color: Color,
    pub hovered_color: Option<Color>,
    pub pressed_color: Option<Color>,
    pub active_color: Option<Color>,
    pub font_size: f32,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            color: Color::TRANSPARENT,
            hovered_color: None,
            pressed_color: None,
            active_color: None,
            font_size: 16.0,
        }
    }
}

pub fn layout(ui_root_id: NodeId, scene: &mut Scene, context: &Context) {
    // Layout ui root
    let Some(root_uibox) = scene.get_mut(ui_root_id).as_uibox_mut() else {
        return;
    };
    let canvas_size = context.display.window_inner_size.as_vec2();
    root_uibox.rect = Rect {
        pos: Vec2::ZERO,
        size: Vec2::new(
            if root_uibox.layout.h_extend {
                canvas_size.x
            } else {
                root_uibox.layout.width
            },
            if root_uibox.layout.v_extend {
                canvas_size.y
            } else {
                root_uibox.layout.height
            },
        ),
    };

    // Recursively layout the whole UI
    fn layout_children(node_id: NodeId, scene: &mut Scene, context: &Context) {
        let Some(uibox) = scene.get_mut(node_id).as_uibox_mut() else {
            return;
        };

        #[derive(Default)]
        struct LayoutInfo {
            layout: Layout,
            axis_pos: f32,
            crossaxis_pos: f32,
            axis_size: f32,
            crossaxis_size: f32,
        }

        let layout_data = uibox.layout.clone();
        let rect = uibox.rect.shrunk(layout_data.padding);
        let dir = layout_data.direction;

        let mut children_data: BTreeMap<NodeId, LayoutInfo> = scene
            .children_of(node_id)
            .iter()
            .copied()
            .filter_map(|id| {
                scene.get(id).as_uibox().map(|uibox| {
                    (
                        id,
                        LayoutInfo {
                            layout: uibox.layout.clone(),
                            ..Default::default()
                        },
                    )
                })
            })
            .collect();

        let axis_size = |rect: Rect| dir.axis_select(rect.size.x, rect.size.y);
        let crossaxis_size = |rect: Rect| dir.axis_select(rect.size.y, rect.size.x);
        let axis_pos = |rect: Rect| dir.axis_select(rect.pos.x, rect.pos.y);
        let crossaxis_pos = |rect: Rect| dir.axis_select(rect.pos.y, rect.pos.x);
        let axis_extend = |layout: &Layout| dir.axis_select(layout.h_extend, layout.v_extend);
        let crossaxis_extend = |layout: &Layout| dir.axis_select(layout.v_extend, layout.h_extend);
        let axis_requested_size = |layout: &Layout| dir.axis_select(layout.width, layout.height);
        let crossaxis_requested_size =
            |layout: &Layout| dir.axis_select(layout.height, layout.width);

        // # Compute children rects
        let mut free_axis_space = axis_size(rect);
        if !children_data.is_empty() {
            let gap_count = children_data.len() - 1;
            free_axis_space -= layout_data.gap * gap_count as f32;
        }
        let mut extend_children_count = 0;

        // ## Compute sizes
        for (_, child_info) in &mut children_data {
            // Determine crossaxis size
            if crossaxis_extend(&child_info.layout) {
                child_info.crossaxis_size = crossaxis_size(rect);
            } else {
                child_info.crossaxis_size = crossaxis_requested_size(&child_info.layout);
            }

            // Set requested axis size
            if axis_extend(&child_info.layout) {
                // Children that extend over the axis are handled after those that don't.
                extend_children_count += 1;
                continue;
            }
            child_info.axis_size = axis_requested_size(&child_info.layout);
            free_axis_space -= child_info.axis_size;
        }
        if free_axis_space > 0.0 {
            for (_, child_info) in &mut children_data {
                // Assign axis size for children that extend over the axis.
                if !axis_extend(&child_info.layout) {
                    continue;
                }

                child_info.axis_size = free_axis_space / extend_children_count as f32;
            }
        }

        // ## Compute positions
        let mut axis_progress = axis_pos(rect);
        for (_, child_info) in &mut children_data {
            child_info.axis_pos = axis_progress;
            axis_progress += child_info.axis_size + layout_data.gap;

            child_info.crossaxis_pos = crossaxis_pos(rect);
        }

        // Apply computed rect to children and recurse
        for (child_id, child_info) in children_data {
            let child_uibox = scene.get_mut(child_id).as_uibox_mut().unwrap();
            child_uibox.rect = Rect {
                pos: Vec2::new(
                    dir.axis_select(child_info.axis_pos, child_info.crossaxis_pos),
                    dir.axis_select(child_info.crossaxis_pos, child_info.axis_pos),
                ),
                size: Vec2::new(
                    dir.axis_select(child_info.axis_size, child_info.crossaxis_size),
                    dir.axis_select(child_info.crossaxis_size, child_info.axis_size),
                ),
            };

            layout_children(child_id, scene, context);
        }
    }

    layout_children(ui_root_id, scene, context);
}

pub fn input(ui_root_id: NodeId, scene: &mut Scene, context: &mut Context) {
    fn gather_ui_nodes(node_id: NodeId, scene: &Scene, ui_nodes: &mut Vec<NodeId>) {
        if scene.get(node_id).as_uibox().is_none() {
            return;
        }
        for &child_id in scene.children_of(node_id) {
            gather_ui_nodes(child_id, scene, ui_nodes);
        }
        ui_nodes.push(node_id);
    }
    let mut ui_nodes = Vec::new();
    gather_ui_nodes(ui_root_id, scene, &mut ui_nodes);

    for node_id in ui_nodes {
        let node = scene.get_mut(node_id);
        if let Some(update_fn) = node.update_fn {
            update_fn(node, context);
        }

        let uibox = node.as_uibox_mut().unwrap();

        if uibox.rect.contains(context.input.pointer_pos) && !context.input.pointer_grabbed {
            if context.input.is_button_pressed(MouseButton::Left) {
                uibox.state = UiBoxState::Pressed;
            } else {
                if uibox.state == UiBoxState::Pressed {
                    if let Some(handler) = uibox.on_click {
                        handler(context);
                    }
                }
                uibox.state = UiBoxState::Hovered;
            }
        } else {
            uibox.state = UiBoxState::Normal;
        }
    }
}

pub fn paint(ui_root_id: NodeId, scene: &Scene, context: &mut Context) {
    fn aux(
        node_id: NodeId,
        scene: &Scene,
        context: &mut Context,
        instances: &mut Vec<UiBoxInstance>,
    ) {
        let Some(uibox) = scene.get(node_id).as_uibox() else {
            return;
        };

        let mut color = match (
            uibox.state,
            uibox.style.hovered_color,
            uibox.style.pressed_color,
        ) {
            (UiBoxState::Hovered, Some(hovered_color), _) => hovered_color,
            (UiBoxState::Pressed, _, Some(pressed_color)) => pressed_color,
            _ => uibox.style.color,
        };

        if let (true, Some(col)) = (uibox.active, uibox.style.active_color) {
            color = col;
        }

        instances.push(UiBoxInstance {
            position: uibox.rect.pos.to_array(),
            size: uibox.rect.size.to_array(),
            color: color.to_array(),
        });

        if let Some(text) = uibox.text.as_ref() {
            let content_rect = uibox.rect.shrunk(uibox.layout.padding);
            context.visual_server.set_text(
                node_id,
                content_rect.pos,
                text.as_bytes(),
                uibox.style.font_size,
                uibox.rect.size.x,
            );
        }

        for &child_id in scene.children_of(node_id) {
            aux(child_id, scene, context, instances);
        }
    }
    let mut instances = Vec::new();
    aux(ui_root_id, scene, context, &mut instances);
    context.visual_server.set_uiboxes(&instances);
}
