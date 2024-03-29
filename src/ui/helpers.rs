use crate::{engine::Context, scene::NodeId, Color, Node, Scene};

use super::{Layout, LayoutDirection, Style, UiBox};

const BUTTON_HEIGHT: f32 = 24.0;
const BUTTON_GROUP_PADDING: f32 = 10.0;
const BUTTON_GROUP_GAP: f32 = 2.0;

pub struct UiBuilder<'a> {
    scene: &'a mut Scene,
    parent: NodeId,
}

impl<'a> UiBuilder<'a> {
    pub fn new(scene: &'a mut Scene) -> Self {
        Self {
            parent: scene.root,
            scene,
        }
    }

    pub fn container(&mut self, node: Node, f: impl FnOnce(&mut UiBuilder)) -> &mut Self {
        let group = self.add_child(node);
        f(&mut UiBuilder {
            scene: self.scene,
            parent: group,
        });
        self
    }

    pub fn v_spacer_big(&mut self) -> &mut Self {
        self.add_child(Node::new_uibox(UiBox {
            layout: Layout {
                height: 20.0,
                ..Default::default()
            },
            ..Default::default()
        }));
        self
    }

    pub fn title(&mut self, text: &str) -> &mut Self {
        self.v_spacer_big();
        self.add_child(Node::new_uibox(UiBox {
            layout: Layout {
                h_extend: true,
                height: 22.0,
                ..Default::default()
            },
            style: Style {
                font_size: 16.0,
                ..Default::default()
            },
            text: Some(String::from(text)),
            ..Default::default()
        }));
        self
    }

    pub fn note(&mut self, text: &str) -> &mut Self {
        self.add_child(Node::new_uibox(UiBox {
            layout: Layout {
                h_extend: true,
                height: 22.0,
                ..Default::default()
            },
            style: Style {
                font_size: 12.0,
                ..Default::default()
            },
            text: Some(String::from(text)),
            ..Default::default()
        }));
        self
    }

    pub fn button_group(&mut self, f: impl FnOnce(&mut UiBuilder)) -> &mut Self {
        let group = self.add_child(Node::new_uibox(UiBox {
            layout: Layout {
                direction: LayoutDirection::Horizontal,
                h_extend: true,
                height: BUTTON_HEIGHT + BUTTON_GROUP_PADDING,
                padding: BUTTON_GROUP_PADDING,
                gap: BUTTON_GROUP_GAP,
                ..Default::default()
            },
            ..Default::default()
        }));
        f(&mut UiBuilder {
            scene: self.scene,
            parent: group,
        });
        self
    }

    pub fn button_list(&mut self, f: impl FnOnce(&mut UiBuilder)) -> &mut Self {
        let list = self.add_child(Node::new_uibox(UiBox {
            layout: Layout {
                direction: LayoutDirection::Vertical,
                h_extend: true,
                height: 1.0, // Gets overriden below.
                padding: BUTTON_GROUP_PADDING,
                gap: BUTTON_GROUP_GAP,
                ..Default::default()
            },
            ..Default::default()
        }));
        f(&mut UiBuilder {
            scene: self.scene,
            parent: list,
        });

        let children_count = self.scene.children_of(list).len();
        let gap_count = children_count.saturating_sub(1) as f32;
        self.scene
            .get_mut(list)
            .as_uibox_mut()
            .unwrap()
            .layout
            .height = children_count as f32 * BUTTON_HEIGHT
            + BUTTON_GROUP_PADDING
            + gap_count * BUTTON_GROUP_GAP;

        self
    }

    pub fn button(
        &mut self,
        text: &str,
        on_click: Option<fn(&mut Context)>,
        update: Option<fn(&mut Node, &mut Context)>,
    ) -> &mut Self {
        let mut node = Node::new_uibox(UiBox {
            layout: Layout {
                h_extend: true,
                height: 22.0,
                padding: 10.0,
                ..Default::default()
            },
            style: Style {
                color: Color::new_rgb(0.18, 0.18, 0.21),
                hovered_color: Some(Color::new_rgb(0.22, 0.22, 0.25)),
                pressed_color: Some(Color::new_rgb(0.16, 0.16, 0.19)),
                active_color: Some(Color::new_rgb(0.3, 0.35, 0.45)),
                font_size: 12.0,
                ..Default::default()
            },
            text: Some(String::from(text)),
            on_click,
            ..Default::default()
        });
        if let Some(update_fn) = update {
            node = node.with_update(update_fn);
        }

        self.add_child(node);

        self
    }

    fn add_child(&mut self, node: Node) -> NodeId {
        self.scene.add_child(self.parent, node)
    }
}
