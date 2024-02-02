use crate::{engine::Context, scene::NodeId, Color, Node, Scene};

use super::{Layout, LayoutDirection, Style, UiBox};

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

    pub fn title(&mut self, text: &str) -> &mut Self {
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

    pub fn button_group(&mut self, f: impl FnOnce(&mut UiBuilder)) -> &mut Self {
        let group = self.add_child(Node::new_uibox(UiBox {
            layout: Layout {
                direction: LayoutDirection::Horizontal,
                h_extend: true,
                height: 24.0 + 10.0 + 20.0, // button height + padding + margin-bottom
                padding: 10.0,
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

    pub fn button(
        &mut self,
        text: &str,
        on_click: Option<fn(&mut Context)>,
        update: Option<fn(&mut Node, &mut Context)>,
    ) -> &mut Self {
        let mut node = Node::new_uibox(UiBox {
            layout: Layout {
                h_extend: true,
                height: 24.0,
                padding: 10.0,
                ..Default::default()
            },
            style: Style {
                color: Color::new_rgb(0.18, 0.18, 0.21),
                hovered_color: Some(Color::new_rgb(0.22, 0.22, 0.25)),
                pressed_color: Some(Color::new_rgb(0.16, 0.16, 0.19)),
                active_color: Some(Color::new_rgb(0.3, 0.35, 0.45)),
                font_size: 14.0,
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
