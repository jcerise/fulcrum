//! The layout pass: anchors + pivots + offsets + stacking. Deliberately small — the whole model
//! fits in one doc page. The solver is pure and unit-tested; the system snapshots the ECS tree,
//! solves, and writes [`UiRect`]s back.

use bevy_ecs::prelude::{Commands, Entity, Query, Res};
use fulcrum_asset::Assets;
use fulcrum_core::{Children, Parent, Rect, Vec2};
use fulcrum_render::{Camera2D, DefaultFont, Font, GlyphCache, WindowInfo};
use rustc_hash::FxHashMap;

use crate::node::{StackDir, UiNode, UiRect, UiSize};
use crate::widgets::{UiButton, UiLabel};

/// Solver input: one node, children by index.
pub(crate) struct SolveNode {
    pub node: UiNode,
    pub children: Vec<usize>,
    /// Intrinsic content size (label text measurement, etc.); folded into `Fit`.
    pub intrinsic: Vec2,
}

/// Measure a node's desired size (bottom-up).
fn measure(nodes: &[SolveNode], index: usize) -> Vec2 {
    let entry = &nodes[index];
    match entry.node.size {
        UiSize::Px(size) => size,
        UiSize::Fill => Vec2::ZERO, // resolved by the parent at place time
        UiSize::Fit => {
            let mut fitted = entry.intrinsic;
            match entry.node.stack {
                StackDir::Vertical(gap) => {
                    let mut height = 0.0;
                    let mut width = 0.0f32;
                    for (i, &child) in entry.children.iter().enumerate() {
                        let child_size = measure(nodes, child) + nodes[child].node.offset;
                        width = width.max(child_size.x);
                        height += child_size.y + if i > 0 { gap } else { 0.0 };
                    }
                    fitted = fitted.max(Vec2::new(width, height));
                }
                StackDir::Horizontal(gap) => {
                    let mut width = 0.0;
                    let mut height = 0.0f32;
                    for (i, &child) in entry.children.iter().enumerate() {
                        let child_size = measure(nodes, child) + nodes[child].node.offset;
                        height = height.max(child_size.y);
                        width += child_size.x + if i > 0 { gap } else { 0.0 };
                    }
                    fitted = fitted.max(Vec2::new(width, height));
                }
                StackDir::None => {
                    for &child in &entry.children {
                        fitted = fitted.max(measure(nodes, child) + nodes[child].node.offset);
                    }
                }
            }
            fitted
        }
    }
}

fn resolve_size(nodes: &[SolveNode], index: usize, parent_size: Vec2, stacked: bool) -> Vec2 {
    match nodes[index].node.size {
        UiSize::Px(size) => size,
        UiSize::Fill => {
            if stacked {
                // In a stack, Fill spans the cross axis and measures the main axis.
                let measured = measure(nodes, index);
                Vec2::new(parent_size.x, measured.y.max(1.0))
            } else {
                parent_size
            }
        }
        UiSize::Fit => measure(nodes, index),
    }
}

/// Place the tree; returns `(index, rect)` in pre-order (also the draw order).
pub(crate) fn solve(nodes: &[SolveNode], roots: &[usize], viewport: Vec2) -> Vec<(usize, Rect)> {
    let mut out = Vec::new();
    let screen = Rect::from_min_size(Vec2::ZERO, viewport);
    for &root in roots {
        place(nodes, root, screen, &mut out);
    }
    out
}

fn place(nodes: &[SolveNode], index: usize, parent: Rect, out: &mut Vec<(usize, Rect)>) {
    let entry = &nodes[index];
    if !entry.node.visible {
        return;
    }
    let size = resolve_size(nodes, index, parent.size(), false);
    let anchor_point = parent.min + entry.node.anchor.fraction() * parent.size();
    let top_left = anchor_point - entry.node.pivot * size + entry.node.offset;
    let rect = Rect::from_min_size(top_left, size);
    out.push((index, rect));
    place_children(nodes, index, rect, out);
}

fn place_children(nodes: &[SolveNode], index: usize, rect: Rect, out: &mut Vec<(usize, Rect)>) {
    let entry = &nodes[index];
    match entry.node.stack {
        StackDir::None => {
            for &child in &entry.children {
                place(nodes, child, rect, out);
            }
        }
        StackDir::Vertical(gap) => {
            let mut cursor = rect.min;
            for &child in &entry.children {
                if !nodes[child].node.visible {
                    continue;
                }
                let size = resolve_size(nodes, child, rect.size(), true);
                let top_left = cursor + nodes[child].node.offset;
                let child_rect = Rect::from_min_size(top_left, size);
                out.push((child, child_rect));
                place_children(nodes, child, child_rect, out);
                cursor.y = child_rect.max.y + gap;
            }
        }
        StackDir::Horizontal(gap) => {
            let mut cursor = rect.min;
            for &child in &entry.children {
                if !nodes[child].node.visible {
                    continue;
                }
                let size = resolve_size(nodes, child, rect.size(), true);
                let top_left = cursor + nodes[child].node.offset;
                let child_rect = Rect::from_min_size(top_left, size);
                out.push((child, child_rect));
                place_children(nodes, child, child_rect, out);
                cursor.x = child_rect.max.x + gap;
            }
        }
    }
}

/// `Update` system: snapshot the UI tree, solve, write [`UiRect`]s.
#[allow(clippy::too_many_arguments, clippy::type_complexity)] // ECS systems legitimately take many resources
pub(crate) fn layout_system(
    mut commands: Commands,
    nodes: Query<(
        Entity,
        &UiNode,
        Option<&Children>,
        Option<&Parent>,
        Option<&UiLabel>,
        Option<&UiButton>,
    )>,
    camera: Option<Res<Camera2D>>,
    window: Option<Res<WindowInfo>>,
    fonts: Option<Res<Assets<Font>>>,
    default_font: Option<Res<DefaultFont>>,
) {
    let (Some(camera), Some(window)) = (camera, window) else {
        return;
    };
    let viewport = camera.ui_size(Vec2::new(window.width as f32, window.height as f32));

    // Snapshot into a flat arena.
    let mut index_of: FxHashMap<Entity, usize> = FxHashMap::default();
    let mut arena: Vec<SolveNode> = Vec::new();
    let mut entities: Vec<Entity> = Vec::new();
    let mut roots: Vec<usize> = Vec::new();
    let mut ordered: Vec<_> = nodes.iter().collect();
    ordered.sort_by_key(|(entity, ..)| *entity); // deterministic root/child ordering

    for (entity, node, _, parent, label, button) in &ordered {
        let mut intrinsic = Vec2::ZERO;
        let text = label
            .map(|l| (l.text.as_str(), l.size))
            .or_else(|| button.map(|b| (b.text.as_str(), b.text_size)));
        if let (Some((text, size)), Some(fonts), Some(default_font)) =
            (text, fonts.as_ref(), default_font.as_ref())
            && let Some(font) = fonts.get(default_font.0)
        {
            intrinsic = GlyphCache::measure(font, text, size);
        }
        index_of.insert(*entity, arena.len());
        entities.push(*entity);
        arena.push(SolveNode {
            node: (*node).clone(),
            children: Vec::new(),
            intrinsic,
        });
        if parent.is_none() {
            roots.push(index_of[entity]);
        }
    }
    for (entity, _, children, ..) in &ordered {
        if let Some(children) = children {
            let parent_index = index_of[entity];
            for child in &children.0 {
                if let Some(&child_index) = index_of.get(child) {
                    arena[parent_index].children.push(child_index);
                }
            }
        }
    }

    for (order, (index, rect)) in solve(&arena, &roots, viewport).into_iter().enumerate() {
        commands.entity(entities[index]).try_insert(UiRect {
            rect,
            order: order as u32,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::Anchor;

    fn node(size: UiSize) -> UiNode {
        UiNode {
            size,
            ..Default::default()
        }
    }

    fn solve_one(nodes: Vec<SolveNode>, roots: Vec<usize>) -> FxHashMap<usize, Rect> {
        solve(&nodes, &roots, Vec2::new(800.0, 600.0))
            .into_iter()
            .collect()
    }

    #[test]
    fn anchored_corners_and_pivots() {
        let mut bottom_right = node(UiSize::Px(Vec2::new(100.0, 40.0)));
        bottom_right.anchor = Anchor::BottomRight;
        bottom_right.pivot = Vec2::new(1.0, 1.0);
        bottom_right.offset = Vec2::new(-8.0, -8.0);
        let rects = solve_one(
            vec![SolveNode {
                node: bottom_right,
                children: vec![],
                intrinsic: Vec2::ZERO,
            }],
            vec![0],
        );
        assert_eq!(
            rects[&0],
            Rect::from_min_size(Vec2::new(692.0, 552.0), Vec2::new(100.0, 40.0))
        );

        let mut centered = node(UiSize::Px(Vec2::new(200.0, 100.0)));
        centered.anchor = Anchor::Center;
        centered.pivot = Vec2::new(0.5, 0.5);
        let rects = solve_one(
            vec![SolveNode {
                node: centered,
                children: vec![],
                intrinsic: Vec2::ZERO,
            }],
            vec![0],
        );
        assert_eq!(rects[&0].center(), Vec2::new(400.0, 300.0));
    }

    #[test]
    fn vertical_stack_with_gap_and_fit_parent() {
        let mut parent = node(UiSize::Fit);
        parent.stack = StackDir::Vertical(4.0);
        parent.offset = Vec2::new(10.0, 10.0);
        let arena = vec![
            SolveNode {
                node: parent,
                children: vec![1, 2],
                intrinsic: Vec2::ZERO,
            },
            SolveNode {
                node: node(UiSize::Px(Vec2::new(120.0, 20.0))),
                children: vec![],
                intrinsic: Vec2::ZERO,
            },
            SolveNode {
                node: node(UiSize::Px(Vec2::new(80.0, 30.0))),
                children: vec![],
                intrinsic: Vec2::ZERO,
            },
        ];
        let rects = solve_one(arena, vec![0]);
        assert_eq!(
            rects[&0].size(),
            Vec2::new(120.0, 54.0),
            "fit = widest x stacked+gap"
        );
        assert_eq!(rects[&1].min, Vec2::new(10.0, 10.0));
        assert_eq!(
            rects[&2].min,
            Vec2::new(10.0, 34.0),
            "below first child + gap"
        );
    }

    #[test]
    fn fill_child_spans_parent_and_hidden_subtrees_are_skipped() {
        let mut parent = node(UiSize::Px(Vec2::new(300.0, 200.0)));
        parent.stack = StackDir::None;
        let mut hidden = node(UiSize::Px(Vec2::new(50.0, 50.0)));
        hidden.visible = false;
        let arena = vec![
            SolveNode {
                node: parent,
                children: vec![1, 2],
                intrinsic: Vec2::ZERO,
            },
            SolveNode {
                node: node(UiSize::Fill),
                children: vec![],
                intrinsic: Vec2::ZERO,
            },
            SolveNode {
                node: hidden,
                children: vec![],
                intrinsic: Vec2::ZERO,
            },
        ];
        let rects = solve_one(arena, vec![0]);
        assert_eq!(
            rects[&1].size(),
            Vec2::new(300.0, 200.0),
            "fill spans parent"
        );
        assert!(!rects.contains_key(&2), "hidden nodes get no rect");
    }

    #[test]
    fn intrinsic_content_drives_fit() {
        let label = SolveNode {
            node: node(UiSize::Fit),
            children: vec![],
            intrinsic: Vec2::new(96.0, 18.0),
        };
        let rects = solve_one(vec![label], vec![0]);
        assert_eq!(rects[&0].size(), Vec2::new(96.0, 18.0));
    }
}
