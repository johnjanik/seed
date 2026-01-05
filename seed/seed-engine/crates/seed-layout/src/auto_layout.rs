//! Auto-layout algorithms for elements without explicit constraints.
//!
//! Provides flexbox-like stack and flow layouts.

use crate::tree::Bounds;

/// Direction for auto-layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    /// Horizontal layout (left to right)
    #[default]
    Horizontal,
    /// Vertical layout (top to bottom)
    Vertical,
}

/// Alignment of items on the cross axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Alignment {
    /// Align to start (left for horizontal, top for vertical)
    #[default]
    Start,
    /// Center on the cross axis
    Center,
    /// Align to end (right for horizontal, bottom for vertical)
    End,
    /// Stretch to fill the container
    Stretch,
}

/// Distribution of items on the main axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Distribution {
    /// Pack items at the start
    #[default]
    Start,
    /// Pack items at the end
    End,
    /// Center items
    Center,
    /// Distribute items with equal space between
    SpaceBetween,
    /// Distribute items with equal space around
    SpaceAround,
    /// Distribute items with equal space evenly
    SpaceEvenly,
}

/// Auto-layout configuration.
#[derive(Debug, Clone, Default)]
pub struct AutoLayout {
    /// Layout direction
    pub direction: Direction,
    /// Gap between items
    pub gap: f64,
    /// Padding inside the container
    pub padding: Padding,
    /// Alignment on the cross axis
    pub alignment: Alignment,
    /// Distribution on the main axis
    pub distribution: Distribution,
    /// Whether to wrap items
    pub wrap: bool,
}

/// Padding on all sides.
#[derive(Debug, Clone, Copy, Default)]
pub struct Padding {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl Padding {
    /// Create uniform padding.
    pub fn uniform(value: f64) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    /// Create symmetric padding.
    pub fn symmetric(horizontal: f64, vertical: f64) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    /// Total horizontal padding.
    pub fn horizontal(&self) -> f64 {
        self.left + self.right
    }

    /// Total vertical padding.
    pub fn vertical(&self) -> f64 {
        self.top + self.bottom
    }
}

/// Size of a child element for layout purposes.
#[derive(Debug, Clone, Copy, Default)]
pub struct ChildSize {
    /// Fixed width (if any)
    pub width: Option<f64>,
    /// Fixed height (if any)
    pub height: Option<f64>,
    /// Minimum width
    pub min_width: f64,
    /// Minimum height
    pub min_height: f64,
    /// Flex grow factor (0 = don't grow)
    pub flex_grow: f64,
    /// Flex shrink factor (1 = can shrink)
    pub flex_shrink: f64,
}

impl AutoLayout {
    /// Create a horizontal stack layout.
    pub fn horizontal() -> Self {
        Self {
            direction: Direction::Horizontal,
            ..Default::default()
        }
    }

    /// Create a vertical stack layout.
    pub fn vertical() -> Self {
        Self {
            direction: Direction::Vertical,
            ..Default::default()
        }
    }

    /// Set the gap between items.
    pub fn with_gap(mut self, gap: f64) -> Self {
        self.gap = gap;
        self
    }

    /// Set uniform padding.
    pub fn with_padding(mut self, padding: f64) -> Self {
        self.padding = Padding::uniform(padding);
        self
    }

    /// Set the alignment.
    pub fn with_alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// Set the distribution.
    pub fn with_distribution(mut self, distribution: Distribution) -> Self {
        self.distribution = distribution;
        self
    }

    /// Enable wrapping.
    pub fn with_wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    /// Compute layout for children within a container.
    pub fn layout(&self, container: Bounds, children: &[ChildSize]) -> Vec<Bounds> {
        if children.is_empty() {
            return Vec::new();
        }

        let content_width = container.width - self.padding.horizontal();
        let content_height = container.height - self.padding.vertical();

        if self.wrap {
            match self.direction {
                Direction::Horizontal => self.layout_horizontal_wrap(
                    container.x + self.padding.left,
                    container.y + self.padding.top,
                    content_width,
                    content_height,
                    children,
                ),
                Direction::Vertical => self.layout_vertical_wrap(
                    container.x + self.padding.left,
                    container.y + self.padding.top,
                    content_width,
                    content_height,
                    children,
                ),
            }
        } else {
            match self.direction {
                Direction::Horizontal => self.layout_horizontal(
                    container.x + self.padding.left,
                    container.y + self.padding.top,
                    content_width,
                    content_height,
                    children,
                ),
                Direction::Vertical => self.layout_vertical(
                    container.x + self.padding.left,
                    container.y + self.padding.top,
                    content_width,
                    content_height,
                    children,
                ),
            }
        }
    }

    fn layout_horizontal(
        &self,
        start_x: f64,
        start_y: f64,
        content_width: f64,
        content_height: f64,
        children: &[ChildSize],
    ) -> Vec<Bounds> {
        let n = children.len();
        let total_gap = self.gap * (n.saturating_sub(1)) as f64;

        // Calculate total fixed and flex sizes
        let mut total_fixed: f64 = 0.0;
        let mut total_flex: f64 = 0.0;
        for child in children {
            if let Some(w) = child.width {
                total_fixed += w;
            } else {
                total_flex += child.flex_grow.max(1.0);
            }
        }

        let available_for_flex = (content_width - total_fixed - total_gap).max(0.0);
        let flex_unit = if total_flex > 0.0 {
            available_for_flex / total_flex
        } else {
            0.0
        };

        // Calculate actual widths
        let widths: Vec<f64> = children
            .iter()
            .map(|child| {
                child.width.unwrap_or_else(|| {
                    (child.flex_grow.max(1.0) * flex_unit).max(child.min_width)
                })
            })
            .collect();

        let total_used: f64 = widths.iter().sum::<f64>() + total_gap;

        // Calculate starting position based on distribution
        let mut x = start_x + match self.distribution {
            Distribution::Start => 0.0,
            Distribution::End => content_width - total_used,
            Distribution::Center => (content_width - total_used) / 2.0,
            Distribution::SpaceBetween | Distribution::SpaceAround | Distribution::SpaceEvenly => 0.0,
        };

        // Calculate spacing for distribution modes
        let extra_space = content_width - total_used;
        let (initial_offset, between_gap) = match self.distribution {
            Distribution::SpaceBetween if n > 1 => (0.0, self.gap + extra_space / (n - 1) as f64),
            Distribution::SpaceAround if n > 0 => {
                let space = extra_space / n as f64;
                (space / 2.0, self.gap + space)
            }
            Distribution::SpaceEvenly if n > 0 => {
                let space = extra_space / (n + 1) as f64;
                (space, self.gap + space)
            }
            _ => (0.0, self.gap),
        };

        x += initial_offset;

        // Layout children
        children
            .iter()
            .zip(widths.iter())
            .map(|(child, &width)| {
                let height = match self.alignment {
                    Alignment::Stretch => content_height,
                    _ => child.height.unwrap_or(child.min_height).min(content_height),
                };

                let y = start_y + match self.alignment {
                    Alignment::Start | Alignment::Stretch => 0.0,
                    Alignment::Center => (content_height - height) / 2.0,
                    Alignment::End => content_height - height,
                };

                let bounds = Bounds::new(x, y, width, height);
                x += width + between_gap;
                bounds
            })
            .collect()
    }

    fn layout_vertical(
        &self,
        start_x: f64,
        start_y: f64,
        content_width: f64,
        content_height: f64,
        children: &[ChildSize],
    ) -> Vec<Bounds> {
        let n = children.len();
        let total_gap = self.gap * (n.saturating_sub(1)) as f64;

        // Calculate total fixed and flex sizes
        let mut total_fixed: f64 = 0.0;
        let mut total_flex: f64 = 0.0;
        for child in children {
            if let Some(h) = child.height {
                total_fixed += h;
            } else {
                total_flex += child.flex_grow.max(1.0);
            }
        }

        let available_for_flex = (content_height - total_fixed - total_gap).max(0.0);
        let flex_unit = if total_flex > 0.0 {
            available_for_flex / total_flex
        } else {
            0.0
        };

        // Calculate actual heights
        let heights: Vec<f64> = children
            .iter()
            .map(|child| {
                child.height.unwrap_or_else(|| {
                    (child.flex_grow.max(1.0) * flex_unit).max(child.min_height)
                })
            })
            .collect();

        let total_used: f64 = heights.iter().sum::<f64>() + total_gap;

        // Calculate starting position based on distribution
        let mut y = start_y + match self.distribution {
            Distribution::Start => 0.0,
            Distribution::End => content_height - total_used,
            Distribution::Center => (content_height - total_used) / 2.0,
            Distribution::SpaceBetween | Distribution::SpaceAround | Distribution::SpaceEvenly => 0.0,
        };

        // Calculate spacing for distribution modes
        let extra_space = content_height - total_used;
        let (initial_offset, between_gap) = match self.distribution {
            Distribution::SpaceBetween if n > 1 => (0.0, self.gap + extra_space / (n - 1) as f64),
            Distribution::SpaceAround if n > 0 => {
                let space = extra_space / n as f64;
                (space / 2.0, self.gap + space)
            }
            Distribution::SpaceEvenly if n > 0 => {
                let space = extra_space / (n + 1) as f64;
                (space, self.gap + space)
            }
            _ => (0.0, self.gap),
        };

        y += initial_offset;

        // Layout children
        children
            .iter()
            .zip(heights.iter())
            .map(|(child, &height)| {
                let width = match self.alignment {
                    Alignment::Stretch => content_width,
                    _ => child.width.unwrap_or(child.min_width).min(content_width),
                };

                let x = start_x + match self.alignment {
                    Alignment::Start | Alignment::Stretch => 0.0,
                    Alignment::Center => (content_width - width) / 2.0,
                    Alignment::End => content_width - width,
                };

                let bounds = Bounds::new(x, y, width, height);
                y += height + between_gap;
                bounds
            })
            .collect()
    }

    fn layout_horizontal_wrap(
        &self,
        start_x: f64,
        start_y: f64,
        content_width: f64,
        content_height: f64,
        children: &[ChildSize],
    ) -> Vec<Bounds> {
        let mut result = Vec::with_capacity(children.len());
        let mut x = start_x;
        let mut y = start_y;
        let mut row_height = 0.0_f64;

        for child in children {
            let width = child.width.unwrap_or(child.min_width);
            let height = child.height.unwrap_or(child.min_height);

            // Check if we need to wrap to next line
            if x + width > start_x + content_width && x > start_x {
                // Move to next row
                x = start_x;
                y += row_height + self.gap;
                row_height = 0.0;
            }

            let final_height = match self.alignment {
                Alignment::Stretch => content_height.min(row_height.max(height)),
                _ => height,
            };

            result.push(Bounds::new(x, y, width, final_height));
            row_height = row_height.max(height);
            x += width + self.gap;
        }

        // Apply cross-axis alignment to each row
        self.apply_row_alignment(&mut result, start_y, content_height);

        result
    }

    fn layout_vertical_wrap(
        &self,
        start_x: f64,
        start_y: f64,
        content_width: f64,
        content_height: f64,
        children: &[ChildSize],
    ) -> Vec<Bounds> {
        let mut result = Vec::with_capacity(children.len());
        let mut x = start_x;
        let mut y = start_y;
        let mut col_width = 0.0_f64;

        for child in children {
            let width = child.width.unwrap_or(child.min_width);
            let height = child.height.unwrap_or(child.min_height);

            // Check if we need to wrap to next column
            if y + height > start_y + content_height && y > start_y {
                // Move to next column
                y = start_y;
                x += col_width + self.gap;
                col_width = 0.0;
            }

            let final_width = match self.alignment {
                Alignment::Stretch => content_width.min(col_width.max(width)),
                _ => width,
            };

            result.push(Bounds::new(x, y, final_width, height));
            col_width = col_width.max(width);
            y += height + self.gap;
        }

        // Apply cross-axis alignment to each column
        self.apply_column_alignment(&mut result, start_x, content_width);

        result
    }

    fn apply_row_alignment(&self, bounds: &mut [Bounds], _start_y: f64, _content_height: f64) {
        if bounds.is_empty() {
            return;
        }

        // Group bounds by rows (same y position)
        let mut i = 0;
        while i < bounds.len() {
            let row_y = bounds[i].y;
            let mut row_height = bounds[i].height;
            let mut j = i + 1;

            // Find all items in this row
            while j < bounds.len() && (bounds[j].y - row_y).abs() < 0.001 {
                row_height = row_height.max(bounds[j].height);
                j += 1;
            }

            // Apply alignment to this row
            for bound in &mut bounds[i..j] {
                match self.alignment {
                    Alignment::Start => {} // Already at start
                    Alignment::Center => {
                        bound.y += (row_height - bound.height) / 2.0;
                    }
                    Alignment::End => {
                        bound.y += row_height - bound.height;
                    }
                    Alignment::Stretch => {
                        bound.height = row_height;
                    }
                }
            }

            i = j;
        }
    }

    fn apply_column_alignment(&self, bounds: &mut [Bounds], _start_x: f64, _content_width: f64) {
        if bounds.is_empty() {
            return;
        }

        // Group bounds by columns (same x position)
        let mut i = 0;
        while i < bounds.len() {
            let col_x = bounds[i].x;
            let mut col_width = bounds[i].width;
            let mut j = i + 1;

            // Find all items in this column
            while j < bounds.len() && (bounds[j].x - col_x).abs() < 0.001 {
                col_width = col_width.max(bounds[j].width);
                j += 1;
            }

            // Apply alignment to this column
            for bound in &mut bounds[i..j] {
                match self.alignment {
                    Alignment::Start => {} // Already at start
                    Alignment::Center => {
                        bound.x += (col_width - bound.width) / 2.0;
                    }
                    Alignment::End => {
                        bound.x += col_width - bound.width;
                    }
                    Alignment::Stretch => {
                        bound.width = col_width;
                    }
                }
            }

            i = j;
        }
    }

    /// Calculate intrinsic size needed to fit all children.
    pub fn intrinsic_size(&self, children: &[ChildSize]) -> (f64, f64) {
        if children.is_empty() {
            return (
                self.padding.horizontal(),
                self.padding.vertical(),
            );
        }

        let n = children.len();
        let total_gap = self.gap * (n.saturating_sub(1)) as f64;

        let (main_size, cross_size) = match self.direction {
            Direction::Horizontal => {
                let width: f64 = children
                    .iter()
                    .map(|c| c.width.unwrap_or(c.min_width))
                    .sum();
                let height = children
                    .iter()
                    .map(|c| c.height.unwrap_or(c.min_height))
                    .fold(0.0_f64, |a, b| a.max(b));
                (width + total_gap, height)
            }
            Direction::Vertical => {
                let width = children
                    .iter()
                    .map(|c| c.width.unwrap_or(c.min_width))
                    .fold(0.0_f64, |a, b| a.max(b));
                let height: f64 = children
                    .iter()
                    .map(|c| c.height.unwrap_or(c.min_height))
                    .sum();
                (width, height + total_gap)
            }
        };

        (
            main_size + self.padding.horizontal(),
            cross_size + self.padding.vertical(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_horizontal_layout() {
        let layout = AutoLayout::horizontal().with_gap(10.0);
        let container = Bounds::new(0.0, 0.0, 300.0, 100.0);
        let children = vec![
            ChildSize { width: Some(50.0), height: Some(50.0), ..Default::default() },
            ChildSize { width: Some(50.0), height: Some(50.0), ..Default::default() },
            ChildSize { width: Some(50.0), height: Some(50.0), ..Default::default() },
        ];

        let result = layout.layout(container, &children);

        assert_eq!(result.len(), 3);
        assert!((result[0].x - 0.0).abs() < 0.001);
        assert!((result[1].x - 60.0).abs() < 0.001); // 50 + 10 gap
        assert!((result[2].x - 120.0).abs() < 0.001); // 50 + 10 + 50 + 10
    }

    #[test]
    fn test_vertical_layout_centered() {
        let layout = AutoLayout::vertical()
            .with_gap(10.0)
            .with_alignment(Alignment::Center);
        let container = Bounds::new(0.0, 0.0, 200.0, 400.0);
        let children = vec![
            ChildSize { width: Some(100.0), height: Some(50.0), ..Default::default() },
            ChildSize { width: Some(100.0), height: Some(50.0), ..Default::default() },
        ];

        let result = layout.layout(container, &children);

        assert_eq!(result.len(), 2);
        // Each child should be centered horizontally (200 - 100) / 2 = 50
        assert!((result[0].x - 50.0).abs() < 0.001);
        assert!((result[1].x - 50.0).abs() < 0.001);
    }

    #[test]
    fn test_flex_grow() {
        let layout = AutoLayout::horizontal();
        let container = Bounds::new(0.0, 0.0, 300.0, 100.0);
        let children = vec![
            ChildSize { width: Some(50.0), height: Some(50.0), ..Default::default() },
            ChildSize { width: None, height: Some(50.0), flex_grow: 1.0, ..Default::default() },
        ];

        let result = layout.layout(container, &children);

        assert_eq!(result.len(), 2);
        assert!((result[0].width - 50.0).abs() < 0.001);
        // Second child should fill remaining space: 300 - 50 = 250
        assert!((result[1].width - 250.0).abs() < 0.001);
    }

    #[test]
    fn test_distribution_space_between() {
        let layout = AutoLayout::horizontal()
            .with_distribution(Distribution::SpaceBetween);
        let container = Bounds::new(0.0, 0.0, 300.0, 100.0);
        let children = vec![
            ChildSize { width: Some(50.0), height: Some(50.0), ..Default::default() },
            ChildSize { width: Some(50.0), height: Some(50.0), ..Default::default() },
            ChildSize { width: Some(50.0), height: Some(50.0), ..Default::default() },
        ];

        let result = layout.layout(container, &children);

        // With space-between: first at 0, last at 250, middle in between
        // Total width used: 150, remaining: 150, gap between 3 items = 150/2 = 75
        assert!((result[0].x - 0.0).abs() < 0.001);
        assert!((result[1].x - 125.0).abs() < 0.001); // 50 + 75
        assert!((result[2].x - 250.0).abs() < 0.001); // 300 - 50
    }

    #[test]
    fn test_intrinsic_size() {
        let layout = AutoLayout::horizontal()
            .with_gap(10.0)
            .with_padding(20.0);
        let children = vec![
            ChildSize { width: Some(50.0), height: Some(30.0), ..Default::default() },
            ChildSize { width: Some(50.0), height: Some(40.0), ..Default::default() },
        ];

        let (width, height) = layout.intrinsic_size(&children);

        // Width: 50 + 10 + 50 + 40 (padding) = 150
        // Height: max(30, 40) + 40 (padding) = 80
        assert!((width - 150.0).abs() < 0.001);
        assert!((height - 80.0).abs() < 0.001);
    }

    #[test]
    fn test_horizontal_wrap() {
        let layout = AutoLayout::horizontal()
            .with_gap(10.0)
            .with_wrap(true);
        let container = Bounds::new(0.0, 0.0, 120.0, 200.0);
        let children = vec![
            ChildSize { width: Some(50.0), height: Some(30.0), ..Default::default() },
            ChildSize { width: Some(50.0), height: Some(30.0), ..Default::default() },
            ChildSize { width: Some(50.0), height: Some(30.0), ..Default::default() },
        ];

        let result = layout.layout(container, &children);

        assert_eq!(result.len(), 3);
        // First row: items 0 and 1 (50 + 10 + 50 = 110 < 120)
        assert!((result[0].x - 0.0).abs() < 0.001);
        assert!((result[0].y - 0.0).abs() < 0.001);
        assert!((result[1].x - 60.0).abs() < 0.001);
        assert!((result[1].y - 0.0).abs() < 0.001);
        // Second row: item 2 (would exceed width if on first row)
        assert!((result[2].x - 0.0).abs() < 0.001);
        assert!((result[2].y - 40.0).abs() < 0.001); // 30 height + 10 gap
    }

    #[test]
    fn test_vertical_wrap() {
        let layout = AutoLayout::vertical()
            .with_gap(10.0)
            .with_wrap(true);
        let container = Bounds::new(0.0, 0.0, 200.0, 80.0);
        let children = vec![
            ChildSize { width: Some(30.0), height: Some(30.0), ..Default::default() },
            ChildSize { width: Some(30.0), height: Some(30.0), ..Default::default() },
            ChildSize { width: Some(30.0), height: Some(30.0), ..Default::default() },
        ];

        let result = layout.layout(container, &children);

        assert_eq!(result.len(), 3);
        // First column: items 0 and 1 (30 + 10 + 30 = 70 < 80)
        assert!((result[0].x - 0.0).abs() < 0.001);
        assert!((result[0].y - 0.0).abs() < 0.001);
        assert!((result[1].x - 0.0).abs() < 0.001);
        assert!((result[1].y - 40.0).abs() < 0.001);
        // Second column: item 2
        assert!((result[2].x - 40.0).abs() < 0.001); // 30 width + 10 gap
        assert!((result[2].y - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_padding() {
        let layout = AutoLayout::horizontal()
            .with_padding(20.0);
        let container = Bounds::new(0.0, 0.0, 200.0, 100.0);
        let children = vec![
            ChildSize { width: Some(50.0), height: Some(50.0), ..Default::default() },
        ];

        let result = layout.layout(container, &children);

        assert_eq!(result.len(), 1);
        // Should start at padding offset
        assert!((result[0].x - 20.0).abs() < 0.001);
        assert!((result[0].y - 20.0).abs() < 0.001);
    }

    #[test]
    fn test_alignment_end() {
        let layout = AutoLayout::horizontal()
            .with_alignment(Alignment::End);
        let container = Bounds::new(0.0, 0.0, 200.0, 100.0);
        let children = vec![
            ChildSize { width: Some(50.0), height: Some(30.0), ..Default::default() },
        ];

        let result = layout.layout(container, &children);

        assert_eq!(result.len(), 1);
        // Should be aligned to bottom (100 - 30 = 70)
        assert!((result[0].y - 70.0).abs() < 0.001);
    }

    #[test]
    fn test_distribution_center() {
        let layout = AutoLayout::horizontal()
            .with_distribution(Distribution::Center);
        let container = Bounds::new(0.0, 0.0, 200.0, 100.0);
        let children = vec![
            ChildSize { width: Some(50.0), height: Some(50.0), ..Default::default() },
        ];

        let result = layout.layout(container, &children);

        assert_eq!(result.len(), 1);
        // Should be centered: (200 - 50) / 2 = 75
        assert!((result[0].x - 75.0).abs() < 0.001);
    }

    #[test]
    fn test_distribution_space_around() {
        let layout = AutoLayout::horizontal()
            .with_distribution(Distribution::SpaceAround);
        let container = Bounds::new(0.0, 0.0, 200.0, 100.0);
        let children = vec![
            ChildSize { width: Some(50.0), height: Some(50.0), ..Default::default() },
            ChildSize { width: Some(50.0), height: Some(50.0), ..Default::default() },
        ];

        let result = layout.layout(container, &children);

        assert_eq!(result.len(), 2);
        // Total used: 100, extra space: 100
        // Space around 2 items: 100/2 = 50 per item, 25 on each side
        // First item: starts at 25
        assert!((result[0].x - 25.0).abs() < 0.001);
    }

    #[test]
    fn test_distribution_space_evenly() {
        let layout = AutoLayout::horizontal()
            .with_distribution(Distribution::SpaceEvenly);
        let container = Bounds::new(0.0, 0.0, 180.0, 100.0);
        let children = vec![
            ChildSize { width: Some(30.0), height: Some(50.0), ..Default::default() },
            ChildSize { width: Some(30.0), height: Some(50.0), ..Default::default() },
            ChildSize { width: Some(30.0), height: Some(50.0), ..Default::default() },
        ];

        let result = layout.layout(container, &children);

        assert_eq!(result.len(), 3);
        // Total used: 90, extra space: 90
        // Space evenly with 3 items: 90/(3+1) = 22.5 between each
        assert!((result[0].x - 22.5).abs() < 0.001);
    }
}
