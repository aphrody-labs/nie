//! The Bevy renderer of the shared UI tree: [`spawn_ui`] turns a [`UiNode`] (built by
//! `nie-ui-tree` from `cfg.bin` data read through the VFS) into Bevy UI entities. Layout is
//! left to Bevy UI, which runs on taffy like `nie_ui_tree::layout`, so the HTML page and the
//! in-canvas Bevy screen come from one tree and one flexbox model.

use bevy_color::Color;
use bevy_ecs::prelude::{Entity, World};
use bevy_text::{FontSize, TextColor, TextFont};
use bevy_ui::widget::Text;
use bevy_ui::{BackgroundColor, FlexDirection, FlexWrap, Node, UiRect, Val};
use nie_ui_tree::{Axis, Rgba, UiNode, UiStyle};

fn color(c: Rgba) -> Color {
    Color::srgba_u8(c[0], c[1], c[2], c[3])
}

fn val(px: Option<f32>) -> Val {
    px.map_or(Val::Auto, Val::Px)
}

/// Bevy layout node for a style (the same flexbox subset as `UiStyle::css` and `to_taffy`).
#[must_use]
pub fn node_for(style: &UiStyle) -> Node {
    Node {
        flex_direction: if style.axis == Axis::Row {
            FlexDirection::Row
        } else {
            FlexDirection::Column
        },
        flex_wrap: if style.wrap {
            FlexWrap::Wrap
        } else {
            FlexWrap::NoWrap
        },
        row_gap: Val::Px(style.gap),
        column_gap: Val::Px(style.gap),
        padding: UiRect::all(Val::Px(style.padding)),
        width: val(style.width),
        height: val(style.height),
        flex_grow: style.grow,
        ..Node::default()
    }
}

/// Spawns the tree under a new root entity and returns it. Text becomes `Text` with its size and
/// colour, boxes become `Node` with their background. Images and links keep a text placeholder
/// (alt text / link text); image decoding to a Bevy `Image` handle is done by `nie-bevy`'s loader.
pub fn spawn_ui(world: &mut World, tree: &UiNode) -> Entity {
    match tree {
        UiNode::Box { style, children } => {
            let mut e = world.spawn(node_for(style));
            if let Some(bg) = style.background {
                e.insert(BackgroundColor(color(bg)));
            }
            let id = e.id();
            let kids: Vec<Entity> = children.iter().map(|c| spawn_ui(world, c)).collect();
            world.entity_mut(id).add_children(&kids);
            id
        }
        UiNode::Text { style, text } | UiNode::Link { style, text, .. } => {
            let mut e = world.spawn((
                Text(text.clone()),
                TextFont::from_font_size(FontSize::Px(style.font_px)),
                node_for(style),
            ));
            if let Some(c) = style.color {
                e.insert(TextColor(color(c)));
            }
            e.id()
        }
        UiNode::Image {
            alt, width, height, ..
        } => world
            .spawn((
                Text(alt.clone()),
                Node {
                    width: Val::Px(*width),
                    height: Val::Px(*height),
                    ..Node::default()
                },
            ))
            .id(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::hierarchy::Children;
    use nie_ui_tree::{Theme, label, page, simple_table};

    #[test]
    fn tree_becomes_entities_with_the_same_structure() {
        let theme = Theme::default();
        let table = simple_table(
            &["a".into(), "b".into()],
            &[vec!["1".into(), "2".into()]],
            100.0,
            &theme,
        );
        let tree = page(&theme, vec![label(20.0, theme.text, "title"), table]);
        let mut world = World::new();
        let root = spawn_ui(&mut world, &tree);
        let nodes = world.query::<&Node>().iter(&world).count();
        assert_eq!(nodes, tree.count(), "one Bevy node per UiNode");
        assert_eq!(
            world.entity(root).get::<Children>().map(|c| c.len()),
            Some(2)
        );
        let root_node = world.entity(root).get::<Node>().unwrap();
        assert_eq!(root_node.flex_direction, FlexDirection::Column);
        assert_eq!(root_node.padding, UiRect::all(Val::Px(24.0)));
        assert_eq!(
            world.entity(root).get::<BackgroundColor>().unwrap().0,
            color(theme.page)
        );
        let texts: Vec<String> = world
            .query::<&Text>()
            .iter(&world)
            .map(|t| t.0.clone())
            .collect();
        assert!(
            texts.contains(&"title".to_owned())
                && texts.contains(&"1".to_owned())
                && texts.contains(&"b".to_owned())
        );
    }

    #[test]
    fn style_maps_to_flex_properties() {
        let n = node_for(&UiStyle {
            axis: Axis::Row,
            gap: 6.0,
            width: Some(120.0),
            grow: 1.0,
            wrap: true,
            ..UiStyle::default()
        });
        assert_eq!(
            (
                n.flex_direction,
                n.flex_wrap,
                n.row_gap,
                n.width,
                n.flex_grow
            ),
            (
                FlexDirection::Row,
                FlexWrap::Wrap,
                Val::Px(6.0),
                Val::Px(120.0),
                1.0
            )
        );
        assert_eq!(n.height, Val::Auto);
    }
}
