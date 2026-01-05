//! Layout computation from document and constraints.

use std::collections::HashMap;

use seed_core::{
    ast::{Element, FrameElement, TextElement, Property, PropertyValue},
    types::ElementId,
    Document, LayoutError,
};
use seed_constraint::{ConstraintSystem, Solution};

use crate::auto_layout::{AutoLayout, Alignment, ChildSize, Padding};
use crate::grid::{GridLayout, GridPlacement, GridChildSize, TrackSize, ItemAlignment};
use crate::text::{measure_text, TextStyle};
use crate::tree::{Bounds, LayoutNode, LayoutNodeId, LayoutTree};

/// Options for layout computation.
#[derive(Debug, Clone)]
pub struct LayoutOptions {
    /// Default width for the root viewport
    pub viewport_width: f64,
    /// Default height for the root viewport
    pub viewport_height: f64,
    /// Default font size for text
    pub default_font_size: f64,
    /// Default line height
    pub default_line_height: f64,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            viewport_width: 800.0,
            viewport_height: 600.0,
            default_font_size: 16.0,
            default_line_height: 1.2,
        }
    }
}

/// Context for layout computation.
struct LayoutContext<'a> {
    options: &'a LayoutOptions,
    tree: LayoutTree,
    constraint_system: ConstraintSystem,
    element_names: HashMap<String, ElementId>,
    name_counter: u64,
}

impl<'a> LayoutContext<'a> {
    fn new(options: &'a LayoutOptions) -> Self {
        Self {
            options,
            tree: LayoutTree::new(),
            constraint_system: ConstraintSystem::new(),
            element_names: HashMap::new(),
            name_counter: 0,
        }
    }

    fn generate_name(&mut self, prefix: &str) -> String {
        self.name_counter += 1;
        format!("{}_{}", prefix, self.name_counter)
    }
}

/// Compute layout for a document.
pub fn compute_layout(doc: &Document, options: &LayoutOptions) -> Result<LayoutTree, LayoutError> {
    let mut ctx = LayoutContext::new(options);

    // First pass: Add document to constraint system
    ctx.constraint_system.add_document(doc)?;

    // Solve constraints
    let solution = ctx.constraint_system.solve()?;

    // Second pass: Build layout tree from solution
    for element in &doc.elements {
        layout_element(&mut ctx, element, None, &solution)?;
    }

    // Compute absolute bounds
    ctx.tree.compute_absolute_bounds();

    Ok(ctx.tree)
}

/// Layout a single element.
fn layout_element(
    ctx: &mut LayoutContext,
    element: &Element,
    parent_id: Option<LayoutNodeId>,
    solution: &Solution,
) -> Result<LayoutNodeId, LayoutError> {
    match element {
        Element::Frame(frame) => layout_frame(ctx, frame, parent_id, solution),
        Element::Text(text) => layout_text(ctx, text, parent_id, solution),
        Element::Svg(svg) => layout_svg(ctx, svg, parent_id, solution),
        Element::Image(image) => layout_image(ctx, image, parent_id, solution),
        Element::Icon(icon) => layout_icon(ctx, icon, parent_id, solution),
        Element::Part(_) => {
            // 3D parts don't have 2D layout
            let node_id = ctx.tree.next_id();
            let node = LayoutNode::new(node_id).with_bounds(Bounds::default());
            if let Some(pid) = parent_id {
                ctx.tree.add_child(pid, node);
            } else {
                ctx.tree.add_root(node);
            }
            Ok(node_id)
        }
        Element::Component(_) => {
            // Components should be expanded before layout
            let node_id = ctx.tree.next_id();
            let node = LayoutNode::new(node_id).with_bounds(Bounds::default());
            if let Some(pid) = parent_id {
                ctx.tree.add_child(pid, node);
            } else {
                ctx.tree.add_root(node);
            }
            Ok(node_id)
        }
        Element::Slot(_) => {
            // Slots should be expanded before layout
            let node_id = ctx.tree.next_id();
            let node = LayoutNode::new(node_id).with_bounds(Bounds::default());
            if let Some(pid) = parent_id {
                ctx.tree.add_child(pid, node);
            } else {
                ctx.tree.add_root(node);
            }
            Ok(node_id)
        }
    }
}

/// Layout a Frame element.
fn layout_frame(
    ctx: &mut LayoutContext,
    frame: &FrameElement,
    parent_id: Option<LayoutNodeId>,
    solution: &Solution,
) -> Result<LayoutNodeId, LayoutError> {
    let name = frame
        .name
        .as_ref()
        .map(|n| n.0.clone())
        .unwrap_or_else(|| ctx.generate_name("frame"));

    // Get bounds from constraint solution or use defaults
    let bounds = get_bounds_from_solution(ctx, &name, solution, parent_id);

    // Get auto-layout settings from properties
    let auto_layout = get_auto_layout_from_properties(&frame.properties);
    let clips = get_bool_property(&frame.properties, "clip").unwrap_or(false);

    let node_id = ctx.tree.next_id();
    let element_id = ElementId(node_id.0);

    let mut node = LayoutNode::new(node_id)
        .with_element_id(element_id)
        .with_name(&name)
        .with_bounds(bounds);
    node.clips_children = clips;

    // Register the element
    ctx.element_names.insert(name.clone(), element_id);

    // Add to tree
    let node_id = if let Some(pid) = parent_id {
        ctx.tree.add_child(pid, node)
    } else {
        ctx.tree.add_root(node)
    };

    // Get grid layout settings from properties
    let grid_layout = get_grid_layout_from_properties(&frame.properties);

    // Layout children
    if !frame.children.is_empty() {
        if let Some(ref grid) = grid_layout {
            // Use grid layout for children
            layout_children_grid(ctx, node_id, &frame.children, grid, solution)?;
        } else if let Some(ref auto) = auto_layout {
            // Use auto-layout for children
            layout_children_auto(ctx, node_id, &frame.children, auto, solution)?;
        } else {
            // Layout children using constraints only
            for child in &frame.children {
                layout_element(ctx, child, Some(node_id), solution)?;
            }
        }
    }

    Ok(node_id)
}

/// Layout a Text element.
fn layout_text(
    ctx: &mut LayoutContext,
    text: &TextElement,
    parent_id: Option<LayoutNodeId>,
    solution: &Solution,
) -> Result<LayoutNodeId, LayoutError> {
    let name = text
        .name
        .as_ref()
        .map(|n| n.0.clone())
        .unwrap_or_else(|| ctx.generate_name("text"));

    // Get text content
    let content = match &text.content {
        seed_core::ast::TextContent::Literal(s) => s.clone(),
        seed_core::ast::TextContent::TokenRef(_) => "[token]".to_string(), // TODO: resolve tokens
    };

    // Get text style from properties
    let style = get_text_style_from_properties(&text.properties, ctx.options);

    // Get max width from parent bounds for wrapping
    let max_width = parent_id
        .and_then(|pid| ctx.tree.get(pid))
        .map(|p| p.bounds.width);

    // Measure text
    let metrics = measure_text(&content, &style, max_width);

    // Get bounds from constraint solution or use measured size
    let mut bounds = get_bounds_from_solution(ctx, &name, solution, parent_id);

    // If width/height not constrained, use measured values
    if bounds.width == 0.0 {
        bounds.width = metrics.width;
    }
    if bounds.height == 0.0 {
        bounds.height = metrics.height;
    }

    let node_id = ctx.tree.next_id();
    let element_id = ElementId(node_id.0);

    let node = LayoutNode::new(node_id)
        .with_element_id(element_id)
        .with_name(&name)
        .with_bounds(bounds);

    ctx.element_names.insert(name.clone(), element_id);

    if let Some(pid) = parent_id {
        ctx.tree.add_child(pid, node);
    } else {
        ctx.tree.add_root(node);
    }

    Ok(node_id)
}

/// Layout an SVG element.
fn layout_svg(
    ctx: &mut LayoutContext,
    svg: &seed_core::ast::SvgElement,
    parent_id: Option<LayoutNodeId>,
    solution: &Solution,
) -> Result<LayoutNodeId, LayoutError> {
    let name = svg.name
        .as_ref()
        .map(|id| id.0.clone())
        .unwrap_or_else(|| ctx.generate_name("svg"));

    // Get bounds from constraint solution or viewBox/properties
    let mut bounds = get_bounds_from_solution(ctx, &name, solution, parent_id);

    // If dimensions not set, try to get from viewBox or default to 24x24 (common icon size)
    if bounds.width == 0.0 {
        bounds.width = svg.view_box.map(|vb| vb.width).unwrap_or(24.0);
    }
    if bounds.height == 0.0 {
        bounds.height = svg.view_box.map(|vb| vb.height).unwrap_or(24.0);
    }

    let node_id = ctx.tree.next_id();
    let element_id = ElementId(node_id.0);

    let node = LayoutNode::new(node_id)
        .with_element_id(element_id)
        .with_name(&name)
        .with_bounds(bounds);

    ctx.element_names.insert(name.clone(), element_id);

    if let Some(pid) = parent_id {
        ctx.tree.add_child(pid, node);
    } else {
        ctx.tree.add_root(node);
    }

    Ok(node_id)
}

/// Layout an Image element.
fn layout_image(
    ctx: &mut LayoutContext,
    image: &seed_core::ast::ImageElement,
    parent_id: Option<LayoutNodeId>,
    solution: &Solution,
) -> Result<LayoutNodeId, LayoutError> {
    let name = image.name
        .as_ref()
        .map(|id| id.0.clone())
        .unwrap_or_else(|| ctx.generate_name("image"));

    // Get bounds from constraint solution or use defaults
    let mut bounds = get_bounds_from_solution(ctx, &name, solution, parent_id);

    // Default image size if not specified (common placeholder size)
    if bounds.width == 0.0 {
        bounds.width = 100.0;
    }
    if bounds.height == 0.0 {
        bounds.height = 100.0;
    }

    let node_id = ctx.tree.next_id();
    let element_id = ElementId(node_id.0);

    let node = LayoutNode::new(node_id)
        .with_element_id(element_id)
        .with_name(&name)
        .with_bounds(bounds);

    ctx.element_names.insert(name.clone(), element_id);

    if let Some(pid) = parent_id {
        ctx.tree.add_child(pid, node);
    } else {
        ctx.tree.add_root(node);
    }

    Ok(node_id)
}

/// Layout an Icon element.
fn layout_icon(
    ctx: &mut LayoutContext,
    icon: &seed_core::ast::IconElement,
    parent_id: Option<LayoutNodeId>,
    solution: &Solution,
) -> Result<LayoutNodeId, LayoutError> {
    let name = icon.name
        .as_ref()
        .map(|id| id.0.clone())
        .unwrap_or_else(|| ctx.generate_name("icon"));

    // Get bounds from constraint solution
    let mut bounds = get_bounds_from_solution(ctx, &name, solution, parent_id);

    // Use icon size if specified, or default to 24x24 (common icon size)
    let icon_size = icon.size
        .as_ref()
        .and_then(|l| l.to_px(None))
        .unwrap_or(24.0);

    if bounds.width == 0.0 {
        bounds.width = icon_size;
    }
    if bounds.height == 0.0 {
        bounds.height = icon_size;
    }

    let node_id = ctx.tree.next_id();
    let element_id = ElementId(node_id.0);

    let node = LayoutNode::new(node_id)
        .with_element_id(element_id)
        .with_name(&name)
        .with_bounds(bounds);

    ctx.element_names.insert(name.clone(), element_id);

    if let Some(pid) = parent_id {
        ctx.tree.add_child(pid, node);
    } else {
        ctx.tree.add_root(node);
    }

    Ok(node_id)
}

/// Get bounds from constraint solution.
fn get_bounds_from_solution(
    ctx: &LayoutContext,
    name: &str,
    solution: &Solution,
    parent_id: Option<LayoutNodeId>,
) -> Bounds {
    // Look up the element ID from the constraint system
    let element_id = ctx.constraint_system.get_element_id(name);

    let (x, y, width, height) = if let Some(eid) = element_id {
        (
            solution.get(eid, "x").unwrap_or(0.0),
            solution.get(eid, "y").unwrap_or(0.0),
            solution.get(eid, "width").unwrap_or(0.0),
            solution.get(eid, "height").unwrap_or(0.0),
        )
    } else {
        // Element not in constraint system, use defaults
        if parent_id.is_none() {
            // Root element defaults to viewport size
            (0.0, 0.0, ctx.options.viewport_width, ctx.options.viewport_height)
        } else {
            (0.0, 0.0, 0.0, 0.0)
        }
    };

    Bounds::new(x, y, width, height)
}

/// Get auto-layout configuration from properties.
fn get_auto_layout_from_properties(properties: &[Property]) -> Option<AutoLayout> {
    let layout_mode = get_string_property(properties, "layout");

    match layout_mode.as_deref() {
        Some("horizontal") | Some("row") => {
            let mut auto = AutoLayout::horizontal();
            apply_auto_layout_properties(&mut auto, properties);
            Some(auto)
        }
        Some("vertical") | Some("column") => {
            let mut auto = AutoLayout::vertical();
            apply_auto_layout_properties(&mut auto, properties);
            Some(auto)
        }
        Some("stack") => {
            // Stack defaults to vertical
            let mut auto = AutoLayout::vertical();
            apply_auto_layout_properties(&mut auto, properties);
            Some(auto)
        }
        _ => None,
    }
}

/// Apply additional auto-layout properties.
fn apply_auto_layout_properties(auto: &mut AutoLayout, properties: &[Property]) {
    if let Some(gap) = get_length_property(properties, "gap") {
        auto.gap = gap;
    }

    if let Some(padding) = get_length_property(properties, "padding") {
        auto.padding = Padding::uniform(padding);
    }

    if let Some(align) = get_string_property(properties, "align") {
        auto.alignment = match align.as_str() {
            "start" => Alignment::Start,
            "center" => Alignment::Center,
            "end" => Alignment::End,
            "stretch" => Alignment::Stretch,
            _ => Alignment::Start,
        };
    }
}

/// Get grid layout configuration from properties.
fn get_grid_layout_from_properties(properties: &[Property]) -> Option<GridLayout> {
    let layout_mode = get_string_property(properties, "layout");

    if layout_mode.as_deref() != Some("grid") {
        return None;
    }

    let mut grid = GridLayout::default();

    // Parse columns: grid-template-columns or columns
    if let Some(columns) = get_grid_tracks_property(properties, "grid-template-columns")
        .or_else(|| get_grid_tracks_property(properties, "columns"))
    {
        grid.columns = columns;
    }

    // Parse rows: grid-template-rows or rows
    if let Some(rows) = get_grid_tracks_property(properties, "grid-template-rows")
        .or_else(|| get_grid_tracks_property(properties, "rows"))
    {
        grid.rows = rows;
    }

    // Parse gap
    if let Some(gap) = get_length_property(properties, "gap") {
        grid.column_gap = gap;
        grid.row_gap = gap;
    }
    if let Some(column_gap) = get_length_property(properties, "column-gap") {
        grid.column_gap = column_gap;
    }
    if let Some(row_gap) = get_length_property(properties, "row-gap") {
        grid.row_gap = row_gap;
    }

    // Parse item alignment
    if let Some(justify) = get_string_property(properties, "justify-items") {
        grid.justify_items = parse_item_alignment(&justify);
    }
    if let Some(align) = get_string_property(properties, "align-items") {
        grid.align_items = parse_item_alignment(&align);
    }

    Some(grid)
}

/// Parse item alignment string.
fn parse_item_alignment(s: &str) -> ItemAlignment {
    match s {
        "start" => ItemAlignment::Start,
        "center" => ItemAlignment::Center,
        "end" => ItemAlignment::End,
        "stretch" => ItemAlignment::Stretch,
        _ => ItemAlignment::Start,
    }
}

/// Get grid tracks from a property value.
fn get_grid_tracks_property(properties: &[Property], name: &str) -> Option<Vec<TrackSize>> {
    properties.iter().find(|p| p.name == name).and_then(|p| {
        match &p.value {
            PropertyValue::GridTracks(tracks) => {
                Some(tracks.iter().map(convert_grid_track_size).collect())
            }
            _ => None,
        }
    })
}

/// Convert AST GridTrackSize to layout GridTrackSize.
fn convert_grid_track_size(ast_track: &seed_core::ast::GridTrackSize) -> TrackSize {
    use seed_core::ast::GridTrackSize as AstTrack;
    match ast_track {
        AstTrack::Fixed(px) => TrackSize::Fixed(*px),
        AstTrack::Fraction(fr) => TrackSize::Fraction(*fr),
        AstTrack::Auto => TrackSize::Auto,
        AstTrack::MinContent => TrackSize::MinContent,
        AstTrack::MaxContent => TrackSize::MaxContent,
        AstTrack::MinMax { min, max } => {
            // Simplify minmax to use numeric bounds
            let min_val = track_to_min_value(min);
            let max_val = track_to_max_value(max);
            TrackSize::MinMax { min: min_val, max: max_val }
        }
        AstTrack::Repeat { count: _, sizes } => {
            // For repeat, we just return the first size (simplified)
            // A proper implementation would expand the repeat
            if let Some(first) = sizes.first() {
                convert_grid_track_size(first)
            } else {
                TrackSize::Auto
            }
        }
    }
}

/// Get minimum value from a track size.
fn track_to_min_value(track: &seed_core::ast::GridTrackSize) -> f64 {
    use seed_core::ast::GridTrackSize as AstTrack;
    match track {
        AstTrack::Fixed(px) => *px,
        AstTrack::Fraction(_) => 0.0,
        AstTrack::Auto | AstTrack::MinContent | AstTrack::MaxContent => 0.0,
        AstTrack::MinMax { min, .. } => track_to_min_value(min),
        AstTrack::Repeat { .. } => 0.0,
    }
}

/// Get maximum value from a track size.
fn track_to_max_value(track: &seed_core::ast::GridTrackSize) -> f64 {
    use seed_core::ast::GridTrackSize as AstTrack;
    match track {
        AstTrack::Fixed(px) => *px,
        AstTrack::Fraction(_) => f64::MAX,
        AstTrack::Auto | AstTrack::MinContent | AstTrack::MaxContent => f64::MAX,
        AstTrack::MinMax { max, .. } => track_to_max_value(max),
        AstTrack::Repeat { .. } => f64::MAX,
    }
}

/// Layout children using grid layout.
fn layout_children_grid(
    ctx: &mut LayoutContext,
    parent_id: LayoutNodeId,
    children: &[Element],
    grid_layout: &GridLayout,
    solution: &Solution,
) -> Result<(), LayoutError> {
    // First, layout all children to get their sizes
    let mut child_ids = Vec::new();
    let mut child_data: Vec<(GridChildSize, GridPlacement)> = Vec::new();

    for (i, child) in children.iter().enumerate() {
        let child_id = layout_element(ctx, child, Some(parent_id), solution)?;
        child_ids.push(child_id);

        let child_node = ctx.tree.get(child_id).unwrap();

        // Get child size
        let child_size = GridChildSize {
            width: if child_node.bounds.width > 0.0 {
                Some(child_node.bounds.width)
            } else {
                None
            },
            height: if child_node.bounds.height > 0.0 {
                Some(child_node.bounds.height)
            } else {
                None
            },
            min_width: 0.0,
            min_height: 0.0,
        };

        // Get placement from child properties
        let placement = get_grid_placement_from_child(child, i);

        child_data.push((child_size, placement));
    }

    // Get parent bounds
    let parent_bounds = ctx.tree.get(parent_id).unwrap().bounds;

    // Compute layout
    let child_bounds = grid_layout.layout(parent_bounds, &child_data);

    // Apply computed bounds to children
    for (child_id, bounds) in child_ids.into_iter().zip(child_bounds.into_iter()) {
        if let Some(child) = ctx.tree.get_mut(child_id) {
            child.bounds = bounds;
        }
    }

    Ok(())
}

/// Get grid placement from a child element's properties.
fn get_grid_placement_from_child(child: &Element, default_index: usize) -> GridPlacement {
    let properties = match child {
        Element::Frame(f) => &f.properties,
        Element::Text(t) => &t.properties,
        Element::Svg(s) => &s.properties,
        _ => return auto_placement(default_index),
    };

    let mut placement = GridPlacement::default();

    // Parse grid-column: "1 / 3" or "1"
    if let Some(col) = get_grid_line_property(properties, "grid-column") {
        placement.column_start = col.0;
        placement.column_end = col.1;
    }

    // Parse grid-row: "1 / 3" or "1"
    if let Some(row) = get_grid_line_property(properties, "grid-row") {
        placement.row_start = row.0;
        placement.row_end = row.1;
    }

    // If no explicit placement, use auto-flow (row-by-row)
    if placement.column_start.is_none() && placement.row_start.is_none() {
        return auto_placement(default_index);
    }

    placement
}

/// Get grid line placement from property.
fn get_grid_line_property(properties: &[Property], name: &str) -> Option<(Option<usize>, Option<usize>)> {
    properties.iter().find(|p| p.name == name).and_then(|p| {
        match &p.value {
            PropertyValue::GridLine(line) => {
                let start = convert_grid_line(&line.start);
                let end = line.end.as_ref().map(convert_grid_line).flatten();
                Some((start, end))
            }
            PropertyValue::Number(n) => {
                // Single number means just the start position
                let n = *n as i32;
                if n > 0 {
                    Some((Some(n as usize), Some(n as usize + 1)))
                } else {
                    None
                }
            }
            _ => None,
        }
    })
}

/// Convert AST GridLine to a line number.
fn convert_grid_line(line: &seed_core::ast::GridLine) -> Option<usize> {
    use seed_core::ast::GridLine;
    match line {
        GridLine::Number(n) if *n > 0 => Some(*n as usize),
        GridLine::Number(_) => None, // Negative numbers would need special handling
        GridLine::Span(n) => Some(*n as usize), // Span is relative, handle specially
        GridLine::Named(_) => None, // Named lines not supported yet
        GridLine::Auto => None,
    }
}

/// Create auto-placement for the given index (simple row-major order).
fn auto_placement(index: usize) -> GridPlacement {
    // Default to row-major auto-placement
    // For now, just use sequential placement (will be improved with actual grid tracking)
    GridPlacement {
        column_start: Some(index + 1),
        column_end: Some(index + 2),
        row_start: Some(1),
        row_end: Some(2),
        ..Default::default()
    }
}

/// Get text style from properties.
fn get_text_style_from_properties(properties: &[Property], options: &LayoutOptions) -> TextStyle {
    TextStyle {
        font_family: get_string_property(properties, "font-family")
            .unwrap_or_else(|| "sans-serif".to_string()),
        font_size: get_length_property(properties, "font-size")
            .unwrap_or(options.default_font_size),
        font_weight: get_number_property(properties, "font-weight")
            .map(|n| n as u16)
            .unwrap_or(400),
        line_height: get_number_property(properties, "line-height")
            .unwrap_or(options.default_line_height),
        letter_spacing: get_length_property(properties, "letter-spacing")
            .unwrap_or(0.0),
    }
}

/// Layout children using auto-layout.
fn layout_children_auto(
    ctx: &mut LayoutContext,
    parent_id: LayoutNodeId,
    children: &[Element],
    auto_layout: &AutoLayout,
    solution: &Solution,
) -> Result<(), LayoutError> {
    // First, layout all children to get their sizes
    let mut child_ids = Vec::new();
    let mut child_sizes = Vec::new();

    for child in children {
        let child_id = layout_element(ctx, child, Some(parent_id), solution)?;
        child_ids.push(child_id);

        let child_node = ctx.tree.get(child_id).unwrap();
        child_sizes.push(ChildSize {
            width: if child_node.bounds.width > 0.0 {
                Some(child_node.bounds.width)
            } else {
                None
            },
            height: if child_node.bounds.height > 0.0 {
                Some(child_node.bounds.height)
            } else {
                None
            },
            min_width: 0.0,
            min_height: 0.0,
            flex_grow: 0.0,
            flex_shrink: 1.0,
        });
    }

    // Get parent bounds
    let parent_bounds = ctx.tree.get(parent_id).unwrap().bounds;

    // Compute layout
    let child_bounds = auto_layout.layout(parent_bounds, &child_sizes);

    // Apply computed bounds to children
    for (child_id, bounds) in child_ids.into_iter().zip(child_bounds.into_iter()) {
        if let Some(child) = ctx.tree.get_mut(child_id) {
            child.bounds = bounds;
        }
    }

    Ok(())
}

// Property accessors

fn get_string_property(properties: &[Property], name: &str) -> Option<String> {
    properties.iter().find(|p| p.name == name).and_then(|p| {
        match &p.value {
            PropertyValue::String(s) => Some(s.clone()),
            PropertyValue::Enum(s) => Some(s.clone()),
            _ => None,
        }
    })
}

fn get_length_property(properties: &[Property], name: &str) -> Option<f64> {
    properties.iter().find(|p| p.name == name).and_then(|p| {
        match &p.value {
            PropertyValue::Length(l) => l.to_px(None),
            PropertyValue::Number(n) => Some(*n),
            _ => None,
        }
    })
}

fn get_number_property(properties: &[Property], name: &str) -> Option<f64> {
    properties.iter().find(|p| p.name == name).and_then(|p| {
        match &p.value {
            PropertyValue::Number(n) => Some(*n),
            PropertyValue::Length(l) => l.to_px(None),
            _ => None,
        }
    })
}

fn get_bool_property(properties: &[Property], name: &str) -> Option<bool> {
    properties.iter().find(|p| p.name == name).and_then(|p| {
        match &p.value {
            PropertyValue::Boolean(b) => Some(*b),
            _ => None,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use seed_core::ast::*;
    use seed_core::types::Identifier;

    fn make_empty_doc() -> Document {
        Document {
            meta: None,
            tokens: None,
            elements: vec![],
            span: Span::default(),
        }
    }

    fn make_frame_element(name: &str, width: f64, height: f64) -> Element {
        Element::Frame(FrameElement {
            name: Some(Identifier(name.to_string())),
            properties: vec![],
            constraints: vec![
                Constraint {
                    kind: ConstraintKind::Equality {
                        property: "width".to_string(),
                        value: Expression::Literal(width),
                    },
                    priority: None,
                    span: Span::default(),
                },
                Constraint {
                    kind: ConstraintKind::Equality {
                        property: "height".to_string(),
                        value: Expression::Literal(height),
                    },
                    priority: None,
                    span: Span::default(),
                },
            ],
            children: vec![],
            span: Span::default(),
        })
    }

    #[test]
    fn test_compute_empty_layout() {
        let doc = make_empty_doc();
        let options = LayoutOptions::default();
        let tree = compute_layout(&doc, &options).unwrap();
        assert_eq!(tree.roots().len(), 0);
    }

    #[test]
    fn test_compute_single_frame() {
        let mut doc = make_empty_doc();
        doc.elements.push(make_frame_element("root", 400.0, 300.0));

        let options = LayoutOptions::default();
        let tree = compute_layout(&doc, &options).unwrap();

        assert_eq!(tree.roots().len(), 1);
        let root = tree.get(tree.roots()[0]).unwrap();
        assert!((root.bounds.width - 400.0).abs() < 0.001);
        assert!((root.bounds.height - 300.0).abs() < 0.001);
    }

    #[test]
    fn test_compute_nested_frames() {
        let child = make_frame_element("child", 100.0, 50.0);

        let parent = Element::Frame(FrameElement {
            name: Some(Identifier("parent".to_string())),
            properties: vec![],
            constraints: vec![
                Constraint {
                    kind: ConstraintKind::Equality {
                        property: "width".to_string(),
                        value: Expression::Literal(400.0),
                    },
                    priority: None,
                    span: Span::default(),
                },
                Constraint {
                    kind: ConstraintKind::Equality {
                        property: "height".to_string(),
                        value: Expression::Literal(300.0),
                    },
                    priority: None,
                    span: Span::default(),
                },
            ],
            children: vec![child],
            span: Span::default(),
        });

        let mut doc = make_empty_doc();
        doc.elements.push(parent);

        let options = LayoutOptions::default();
        let tree = compute_layout(&doc, &options).unwrap();

        assert_eq!(tree.roots().len(), 1);

        let parent_node = tree.get(tree.roots()[0]).unwrap();
        assert_eq!(parent_node.children.len(), 1);
    }

    #[test]
    fn test_compute_multiple_children() {
        let child1 = make_frame_element("child1", 50.0, 30.0);
        let child2 = make_frame_element("child2", 60.0, 40.0);
        let child3 = make_frame_element("child3", 70.0, 50.0);

        let parent = Element::Frame(FrameElement {
            name: Some(Identifier("parent".to_string())),
            properties: vec![],
            constraints: vec![
                Constraint {
                    kind: ConstraintKind::Equality {
                        property: "width".to_string(),
                        value: Expression::Literal(300.0),
                    },
                    priority: None,
                    span: Span::default(),
                },
                Constraint {
                    kind: ConstraintKind::Equality {
                        property: "height".to_string(),
                        value: Expression::Literal(200.0),
                    },
                    priority: None,
                    span: Span::default(),
                },
            ],
            children: vec![child1, child2, child3],
            span: Span::default(),
        });

        let mut doc = make_empty_doc();
        doc.elements.push(parent);

        let options = LayoutOptions::default();
        let tree = compute_layout(&doc, &options).unwrap();

        let parent_node = tree.get(tree.roots()[0]).unwrap();
        assert_eq!(parent_node.children.len(), 3, "Parent should have 3 children");

        // Verify each child has correct dimensions
        for (i, &child_id) in parent_node.children.iter().enumerate() {
            let child = tree.get(child_id).unwrap();
            let expected_width = 50.0 + (i as f64) * 10.0;
            let expected_height = 30.0 + (i as f64) * 10.0;
            assert!(
                (child.bounds.width - expected_width).abs() < 0.001,
                "Child {} width mismatch: expected {}, got {}",
                i, expected_width, child.bounds.width
            );
            assert!(
                (child.bounds.height - expected_height).abs() < 0.001,
                "Child {} height mismatch: expected {}, got {}",
                i, expected_height, child.bounds.height
            );
        }
    }

    #[test]
    fn test_compute_deeply_nested_frames() {
        // Create 5 levels of nesting
        let level4 = make_frame_element("level4", 10.0, 10.0);

        let level3 = Element::Frame(FrameElement {
            name: Some(Identifier("level3".to_string())),
            properties: vec![],
            constraints: vec![
                Constraint {
                    kind: ConstraintKind::Equality {
                        property: "width".to_string(),
                        value: Expression::Literal(20.0),
                    },
                    priority: None,
                    span: Span::default(),
                },
                Constraint {
                    kind: ConstraintKind::Equality {
                        property: "height".to_string(),
                        value: Expression::Literal(20.0),
                    },
                    priority: None,
                    span: Span::default(),
                },
            ],
            children: vec![level4],
            span: Span::default(),
        });

        let level2 = Element::Frame(FrameElement {
            name: Some(Identifier("level2".to_string())),
            properties: vec![],
            constraints: vec![
                Constraint {
                    kind: ConstraintKind::Equality {
                        property: "width".to_string(),
                        value: Expression::Literal(40.0),
                    },
                    priority: None,
                    span: Span::default(),
                },
                Constraint {
                    kind: ConstraintKind::Equality {
                        property: "height".to_string(),
                        value: Expression::Literal(40.0),
                    },
                    priority: None,
                    span: Span::default(),
                },
            ],
            children: vec![level3],
            span: Span::default(),
        });

        let level1 = Element::Frame(FrameElement {
            name: Some(Identifier("level1".to_string())),
            properties: vec![],
            constraints: vec![
                Constraint {
                    kind: ConstraintKind::Equality {
                        property: "width".to_string(),
                        value: Expression::Literal(80.0),
                    },
                    priority: None,
                    span: Span::default(),
                },
                Constraint {
                    kind: ConstraintKind::Equality {
                        property: "height".to_string(),
                        value: Expression::Literal(80.0),
                    },
                    priority: None,
                    span: Span::default(),
                },
            ],
            children: vec![level2],
            span: Span::default(),
        });

        let root = Element::Frame(FrameElement {
            name: Some(Identifier("root".to_string())),
            properties: vec![],
            constraints: vec![
                Constraint {
                    kind: ConstraintKind::Equality {
                        property: "width".to_string(),
                        value: Expression::Literal(160.0),
                    },
                    priority: None,
                    span: Span::default(),
                },
                Constraint {
                    kind: ConstraintKind::Equality {
                        property: "height".to_string(),
                        value: Expression::Literal(160.0),
                    },
                    priority: None,
                    span: Span::default(),
                },
            ],
            children: vec![level1],
            span: Span::default(),
        });

        let mut doc = make_empty_doc();
        doc.elements.push(root);

        let options = LayoutOptions::default();
        let tree = compute_layout(&doc, &options).unwrap();

        // Verify the structure is correct
        let root_node = tree.get(tree.roots()[0]).unwrap();
        assert_eq!(root_node.children.len(), 1, "Root should have 1 child");

        let l1_node = tree.get(root_node.children[0]).unwrap();
        assert_eq!(l1_node.children.len(), 1, "Level1 should have 1 child");

        let l2_node = tree.get(l1_node.children[0]).unwrap();
        assert_eq!(l2_node.children.len(), 1, "Level2 should have 1 child");

        let l3_node = tree.get(l2_node.children[0]).unwrap();
        assert_eq!(l3_node.children.len(), 1, "Level3 should have 1 child");

        let l4_node = tree.get(l3_node.children[0]).unwrap();
        assert_eq!(l4_node.children.len(), 0, "Level4 should have 0 children");

        // Verify dimensions
        assert!((l4_node.bounds.width - 10.0).abs() < 0.001);
        assert!((l4_node.bounds.height - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_compute_complex_tree() {
        // Build a complex tree structure:
        // root
        // ├── branch1
        // │   ├── leaf1a
        // │   └── leaf1b
        // └── branch2
        //     ├── leaf2a
        //     └── leaf2b

        let leaf1a = make_frame_element("leaf1a", 20.0, 20.0);
        let leaf1b = make_frame_element("leaf1b", 25.0, 25.0);
        let leaf2a = make_frame_element("leaf2a", 30.0, 30.0);
        let leaf2b = make_frame_element("leaf2b", 35.0, 35.0);

        let branch1 = Element::Frame(FrameElement {
            name: Some(Identifier("branch1".to_string())),
            properties: vec![],
            constraints: vec![
                Constraint {
                    kind: ConstraintKind::Equality {
                        property: "width".to_string(),
                        value: Expression::Literal(100.0),
                    },
                    priority: None,
                    span: Span::default(),
                },
                Constraint {
                    kind: ConstraintKind::Equality {
                        property: "height".to_string(),
                        value: Expression::Literal(80.0),
                    },
                    priority: None,
                    span: Span::default(),
                },
            ],
            children: vec![leaf1a, leaf1b],
            span: Span::default(),
        });

        let branch2 = Element::Frame(FrameElement {
            name: Some(Identifier("branch2".to_string())),
            properties: vec![],
            constraints: vec![
                Constraint {
                    kind: ConstraintKind::Equality {
                        property: "width".to_string(),
                        value: Expression::Literal(120.0),
                    },
                    priority: None,
                    span: Span::default(),
                },
                Constraint {
                    kind: ConstraintKind::Equality {
                        property: "height".to_string(),
                        value: Expression::Literal(100.0),
                    },
                    priority: None,
                    span: Span::default(),
                },
            ],
            children: vec![leaf2a, leaf2b],
            span: Span::default(),
        });

        let root = Element::Frame(FrameElement {
            name: Some(Identifier("root".to_string())),
            properties: vec![],
            constraints: vec![
                Constraint {
                    kind: ConstraintKind::Equality {
                        property: "width".to_string(),
                        value: Expression::Literal(300.0),
                    },
                    priority: None,
                    span: Span::default(),
                },
                Constraint {
                    kind: ConstraintKind::Equality {
                        property: "height".to_string(),
                        value: Expression::Literal(250.0),
                    },
                    priority: None,
                    span: Span::default(),
                },
            ],
            children: vec![branch1, branch2],
            span: Span::default(),
        });

        let mut doc = make_empty_doc();
        doc.elements.push(root);

        let options = LayoutOptions::default();
        let tree = compute_layout(&doc, &options).unwrap();

        // Count total nodes
        let total_nodes = tree.nodes().count();
        assert_eq!(total_nodes, 7, "Should have 7 total nodes: root + 2 branches + 4 leaves");

        // Verify root has 2 children (branches)
        let root_node = tree.get(tree.roots()[0]).unwrap();
        assert_eq!(root_node.children.len(), 2, "Root should have 2 children");

        // Verify each branch has 2 children (leaves)
        for &branch_id in &root_node.children {
            let branch = tree.get(branch_id).unwrap();
            assert_eq!(branch.children.len(), 2, "Each branch should have 2 leaves");
        }
    }

    // Grid layout tests

    fn make_grid_frame(name: &str, width: f64, height: f64, columns: Vec<seed_core::ast::GridTrackSize>, rows: Vec<seed_core::ast::GridTrackSize>, children: Vec<Element>) -> Element {
        Element::Frame(FrameElement {
            name: Some(Identifier(name.to_string())),
            properties: vec![
                Property {
                    name: "layout".to_string(),
                    value: PropertyValue::Enum("grid".to_string()),
                    span: Span::default(),
                },
                Property {
                    name: "grid-template-columns".to_string(),
                    value: PropertyValue::GridTracks(columns),
                    span: Span::default(),
                },
                Property {
                    name: "grid-template-rows".to_string(),
                    value: PropertyValue::GridTracks(rows),
                    span: Span::default(),
                },
            ],
            constraints: vec![
                Constraint {
                    kind: ConstraintKind::Equality {
                        property: "width".to_string(),
                        value: Expression::Literal(width),
                    },
                    priority: None,
                    span: Span::default(),
                },
                Constraint {
                    kind: ConstraintKind::Equality {
                        property: "height".to_string(),
                        value: Expression::Literal(height),
                    },
                    priority: None,
                    span: Span::default(),
                },
            ],
            children,
            span: Span::default(),
        })
    }

    #[test]
    fn test_compute_grid_basic() {
        use seed_core::ast::GridTrackSize;

        let child1 = make_frame_element("child1", 0.0, 0.0); // Let grid determine size
        let child2 = make_frame_element("child2", 0.0, 0.0);

        let grid = make_grid_frame(
            "grid",
            200.0,
            100.0,
            vec![GridTrackSize::Fixed(100.0), GridTrackSize::Fixed(100.0)],
            vec![GridTrackSize::Fixed(100.0)],
            vec![child1, child2],
        );

        let mut doc = make_empty_doc();
        doc.elements.push(grid);

        let options = LayoutOptions::default();
        let tree = compute_layout(&doc, &options).unwrap();

        let root = tree.get(tree.roots()[0]).unwrap();
        assert_eq!(root.children.len(), 2);

        // First child should be in first column
        let c1 = tree.get(root.children[0]).unwrap();
        assert!((c1.bounds.x - 0.0).abs() < 1.0, "Child 1 x should be 0, got {}", c1.bounds.x);

        // Second child should be in second column
        let c2 = tree.get(root.children[1]).unwrap();
        assert!((c2.bounds.x - 100.0).abs() < 1.0, "Child 2 x should be 100, got {}", c2.bounds.x);
    }

    #[test]
    fn test_compute_grid_fractional() {
        use seed_core::ast::GridTrackSize;

        let child1 = make_frame_element("child1", 0.0, 0.0);
        let child2 = make_frame_element("child2", 0.0, 0.0);
        let child3 = make_frame_element("child3", 0.0, 0.0);

        let grid = make_grid_frame(
            "grid",
            300.0,
            100.0,
            vec![
                GridTrackSize::Fraction(1.0),
                GridTrackSize::Fraction(2.0),
                GridTrackSize::Fraction(1.0),
            ],
            vec![GridTrackSize::Fixed(100.0)],
            vec![child1, child2, child3],
        );

        let mut doc = make_empty_doc();
        doc.elements.push(grid);

        let options = LayoutOptions::default();
        let tree = compute_layout(&doc, &options).unwrap();

        let root = tree.get(tree.roots()[0]).unwrap();
        assert_eq!(root.children.len(), 3);

        // 1fr : 2fr : 1fr = 75px : 150px : 75px
        let c1 = tree.get(root.children[0]).unwrap();
        let c2 = tree.get(root.children[1]).unwrap();
        let c3 = tree.get(root.children[2]).unwrap();

        assert!((c1.bounds.width - 75.0).abs() < 1.0, "Child 1 width should be 75, got {}", c1.bounds.width);
        assert!((c2.bounds.width - 150.0).abs() < 1.0, "Child 2 width should be 150, got {}", c2.bounds.width);
        assert!((c3.bounds.width - 75.0).abs() < 1.0, "Child 3 width should be 75, got {}", c3.bounds.width);

        // Check x positions
        assert!((c1.bounds.x - 0.0).abs() < 1.0);
        assert!((c2.bounds.x - 75.0).abs() < 1.0);
        assert!((c3.bounds.x - 225.0).abs() < 1.0);
    }

    #[test]
    fn test_compute_grid_with_gap() {
        use seed_core::ast::GridTrackSize;

        let child1 = make_frame_element("child1", 0.0, 0.0);
        let child2 = make_frame_element("child2", 0.0, 0.0);

        // Create a grid with gap
        let grid = Element::Frame(FrameElement {
            name: Some(Identifier("grid".to_string())),
            properties: vec![
                Property {
                    name: "layout".to_string(),
                    value: PropertyValue::Enum("grid".to_string()),
                    span: Span::default(),
                },
                Property {
                    name: "grid-template-columns".to_string(),
                    value: PropertyValue::GridTracks(vec![
                        GridTrackSize::Fixed(90.0),
                        GridTrackSize::Fixed(90.0),
                    ]),
                    span: Span::default(),
                },
                Property {
                    name: "gap".to_string(),
                    value: PropertyValue::Length(seed_core::types::Length { value: 20.0, unit: seed_core::types::LengthUnit::Px }),
                    span: Span::default(),
                },
            ],
            constraints: vec![
                Constraint {
                    kind: ConstraintKind::Equality {
                        property: "width".to_string(),
                        value: Expression::Literal(200.0),
                    },
                    priority: None,
                    span: Span::default(),
                },
                Constraint {
                    kind: ConstraintKind::Equality {
                        property: "height".to_string(),
                        value: Expression::Literal(100.0),
                    },
                    priority: None,
                    span: Span::default(),
                },
            ],
            children: vec![child1, child2],
            span: Span::default(),
        });

        let mut doc = make_empty_doc();
        doc.elements.push(grid);

        let options = LayoutOptions::default();
        let tree = compute_layout(&doc, &options).unwrap();

        let root = tree.get(tree.roots()[0]).unwrap();
        assert_eq!(root.children.len(), 2);

        let c1 = tree.get(root.children[0]).unwrap();
        let c2 = tree.get(root.children[1]).unwrap();

        assert!((c1.bounds.x - 0.0).abs() < 1.0);
        assert!((c2.bounds.x - 110.0).abs() < 1.0, "Child 2 x should be 110 (90 + 20 gap), got {}", c2.bounds.x);
    }
}
