//! CSS Grid-like layout algorithm.
//!
//! Provides 2D grid layout with explicit and implicit tracks.

use crate::tree::Bounds;

/// Grid layout configuration.
#[derive(Debug, Clone, Default)]
pub struct GridLayout {
    /// Column track definitions
    pub columns: Vec<TrackSize>,
    /// Row track definitions
    pub rows: Vec<TrackSize>,
    /// Gap between columns
    pub column_gap: f64,
    /// Gap between rows
    pub row_gap: f64,
    /// Alignment of items within their cell (horizontal)
    pub justify_items: ItemAlignment,
    /// Alignment of items within their cell (vertical)
    pub align_items: ItemAlignment,
    /// Alignment of the grid within the container (horizontal)
    pub justify_content: ContentAlignment,
    /// Alignment of the grid within the container (vertical)
    pub align_content: ContentAlignment,
    /// Size for auto-created columns
    pub auto_columns: TrackSize,
    /// Size for auto-created rows
    pub auto_rows: TrackSize,
}

/// Track sizing for columns/rows.
#[derive(Debug, Clone, Copy)]
pub enum TrackSize {
    /// Fixed size in pixels
    Fixed(f64),
    /// Fraction of available space (fr units)
    Fraction(f64),
    /// Size based on content (min-content)
    MinContent,
    /// Size based on content (max-content)
    MaxContent,
    /// Automatic sizing
    Auto,
    /// Minimum and maximum bounds
    MinMax { min: f64, max: f64 },
}

impl Default for TrackSize {
    fn default() -> Self {
        Self::Auto
    }
}

/// Item alignment within grid cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ItemAlignment {
    /// Align to start
    #[default]
    Start,
    /// Align to center
    Center,
    /// Align to end
    End,
    /// Stretch to fill cell
    Stretch,
}

/// Content alignment for the grid within container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContentAlignment {
    /// Pack at start
    #[default]
    Start,
    /// Pack at center
    Center,
    /// Pack at end
    End,
    /// Distribute with space between
    SpaceBetween,
    /// Distribute with space around
    SpaceAround,
    /// Distribute with equal space
    SpaceEvenly,
    /// Stretch to fill
    Stretch,
}

/// Grid item placement.
#[derive(Debug, Clone, Default)]
pub struct GridPlacement {
    /// Column start (1-indexed, None for auto)
    pub column_start: Option<usize>,
    /// Column end (exclusive, None for span 1)
    pub column_end: Option<usize>,
    /// Row start (1-indexed, None for auto)
    pub row_start: Option<usize>,
    /// Row end (exclusive, None for span 1)
    pub row_end: Option<usize>,
    /// Alignment override for this item (horizontal)
    pub justify_self: Option<ItemAlignment>,
    /// Alignment override for this item (vertical)
    pub align_self: Option<ItemAlignment>,
}

impl GridPlacement {
    /// Create a placement for a specific cell.
    pub fn cell(column: usize, row: usize) -> Self {
        Self {
            column_start: Some(column),
            column_end: Some(column + 1),
            row_start: Some(row),
            row_end: Some(row + 1),
            ..Default::default()
        }
    }

    /// Create a placement spanning multiple columns.
    pub fn span_columns(column: usize, span: usize, row: usize) -> Self {
        Self {
            column_start: Some(column),
            column_end: Some(column + span),
            row_start: Some(row),
            row_end: Some(row + 1),
            ..Default::default()
        }
    }

    /// Create a placement spanning multiple rows.
    pub fn span_rows(column: usize, row: usize, span: usize) -> Self {
        Self {
            column_start: Some(column),
            column_end: Some(column + 1),
            row_start: Some(row),
            row_end: Some(row + span),
            ..Default::default()
        }
    }

    /// Get the column span.
    pub fn column_span(&self) -> usize {
        match (self.column_start, self.column_end) {
            (Some(start), Some(end)) => end.saturating_sub(start).max(1),
            _ => 1,
        }
    }

    /// Get the row span.
    pub fn row_span(&self) -> usize {
        match (self.row_start, self.row_end) {
            (Some(start), Some(end)) => end.saturating_sub(start).max(1),
            _ => 1,
        }
    }
}

/// Child size info for grid items.
#[derive(Debug, Clone, Copy, Default)]
pub struct GridChildSize {
    /// Fixed width (if any)
    pub width: Option<f64>,
    /// Fixed height (if any)
    pub height: Option<f64>,
    /// Minimum width
    pub min_width: f64,
    /// Minimum height
    pub min_height: f64,
}

impl GridLayout {
    /// Create a new grid layout with specified columns and rows.
    pub fn new(columns: Vec<TrackSize>, rows: Vec<TrackSize>) -> Self {
        Self {
            columns,
            rows,
            ..Default::default()
        }
    }

    /// Create a grid with equal-sized columns.
    pub fn columns(count: usize, size: TrackSize) -> Self {
        Self {
            columns: vec![size; count],
            rows: vec![TrackSize::Auto],
            ..Default::default()
        }
    }

    /// Create a grid with equal-sized rows.
    pub fn rows(count: usize, size: TrackSize) -> Self {
        Self {
            columns: vec![TrackSize::Auto],
            rows: vec![size; count],
            ..Default::default()
        }
    }

    /// Set the gap between items.
    pub fn with_gap(mut self, gap: f64) -> Self {
        self.column_gap = gap;
        self.row_gap = gap;
        self
    }

    /// Set item alignment.
    pub fn with_alignment(mut self, justify: ItemAlignment, align: ItemAlignment) -> Self {
        self.justify_items = justify;
        self.align_items = align;
        self
    }

    /// Set content alignment.
    pub fn with_content_alignment(
        mut self,
        justify: ContentAlignment,
        align: ContentAlignment,
    ) -> Self {
        self.justify_content = justify;
        self.align_content = align;
        self
    }

    /// Compute layout for grid items.
    pub fn layout(
        &self,
        container: Bounds,
        children: &[(GridChildSize, GridPlacement)],
    ) -> Vec<Bounds> {
        if children.is_empty() {
            return Vec::new();
        }

        // Determine grid dimensions
        let (num_cols, num_rows) = self.calculate_grid_size(children);

        if num_cols == 0 || num_rows == 0 {
            return vec![Bounds::default(); children.len()];
        }

        // Resolve track sizes
        let column_sizes = self.resolve_tracks(
            &self.columns,
            num_cols,
            container.width,
            self.column_gap,
            &self.auto_columns,
        );
        let row_sizes = self.resolve_tracks(
            &self.rows,
            num_rows,
            container.height,
            self.row_gap,
            &self.auto_rows,
        );

        // Calculate track positions
        let column_positions = self.calculate_positions(&column_sizes, self.column_gap, container.x);
        let row_positions = self.calculate_positions(&row_sizes, self.row_gap, container.y);

        // Layout each child
        children
            .iter()
            .map(|(child_size, placement)| {
                let col_start = placement.column_start.unwrap_or(1).saturating_sub(1);
                let col_end = placement.column_end.unwrap_or(col_start + 2).saturating_sub(1);
                let row_start = placement.row_start.unwrap_or(1).saturating_sub(1);
                let row_end = placement.row_end.unwrap_or(row_start + 2).saturating_sub(1);

                // Get cell bounds
                let cell_x = column_positions.get(col_start).copied().unwrap_or(container.x);
                let cell_y = row_positions.get(row_start).copied().unwrap_or(container.y);

                let cell_width: f64 = (col_start..col_end.min(num_cols))
                    .map(|i| column_sizes.get(i).copied().unwrap_or(0.0))
                    .sum::<f64>()
                    + self.column_gap * (col_end.saturating_sub(col_start).saturating_sub(1)) as f64;

                let cell_height: f64 = (row_start..row_end.min(num_rows))
                    .map(|i| row_sizes.get(i).copied().unwrap_or(0.0))
                    .sum::<f64>()
                    + self.row_gap * (row_end.saturating_sub(row_start).saturating_sub(1)) as f64;

                // Apply item alignment
                let justify = placement.justify_self.unwrap_or(self.justify_items);
                let align = placement.align_self.unwrap_or(self.align_items);

                // If no explicit size, use cell size (filling the cell by default)
                let item_width = match justify {
                    ItemAlignment::Stretch => cell_width,
                    _ => child_size.width.unwrap_or(cell_width).min(cell_width),
                };

                let item_height = match align {
                    ItemAlignment::Stretch => cell_height,
                    _ => child_size.height.unwrap_or(cell_height).min(cell_height),
                };

                let x = cell_x + match justify {
                    ItemAlignment::Start | ItemAlignment::Stretch => 0.0,
                    ItemAlignment::Center => (cell_width - item_width) / 2.0,
                    ItemAlignment::End => cell_width - item_width,
                };

                let y = cell_y + match align {
                    ItemAlignment::Start | ItemAlignment::Stretch => 0.0,
                    ItemAlignment::Center => (cell_height - item_height) / 2.0,
                    ItemAlignment::End => cell_height - item_height,
                };

                Bounds::new(x, y, item_width, item_height)
            })
            .collect()
    }

    fn calculate_grid_size(&self, children: &[(GridChildSize, GridPlacement)]) -> (usize, usize) {
        let mut max_col = self.columns.len();
        let mut max_row = self.rows.len();

        for (_size, placement) in children {
            if let Some(end) = placement.column_end {
                max_col = max_col.max(end.saturating_sub(1));
            } else if let Some(start) = placement.column_start {
                max_col = max_col.max(start);
            }
            if let Some(end) = placement.row_end {
                max_row = max_row.max(end.saturating_sub(1));
            } else if let Some(start) = placement.row_start {
                max_row = max_row.max(start);
            }
        }

        // Ensure at least 1 column and row if we have children
        if !children.is_empty() {
            max_col = max_col.max(1);
            max_row = max_row.max(1);
        }

        (max_col, max_row)
    }

    fn resolve_tracks(
        &self,
        defined: &[TrackSize],
        count: usize,
        available: f64,
        gap: f64,
        auto_size: &TrackSize,
    ) -> Vec<f64> {
        let total_gap = gap * (count.saturating_sub(1)) as f64;
        let available_for_tracks = (available - total_gap).max(0.0);

        // Extend tracks to required count
        let tracks: Vec<TrackSize> = (0..count)
            .map(|i| {
                if i < defined.len() {
                    defined[i]
                } else {
                    *auto_size
                }
            })
            .collect();

        // Calculate fixed and fraction totals
        let mut fixed_total = 0.0_f64;
        let mut fraction_total = 0.0_f64;
        let mut auto_count = 0usize;

        for track in &tracks {
            match track {
                TrackSize::Fixed(size) => fixed_total += size,
                TrackSize::Fraction(fr) => fraction_total += fr,
                TrackSize::MinMax { min, .. } => fixed_total += min,
                TrackSize::Auto | TrackSize::MinContent | TrackSize::MaxContent => auto_count += 1,
            }
        }

        let remaining = (available_for_tracks - fixed_total).max(0.0);

        // Calculate fr unit - fractions get their share of remaining space
        let fr_unit = if fraction_total > 0.0 {
            remaining / fraction_total
        } else {
            0.0
        };

        // Auto tracks share any remaining space after fractions
        let auto_unit = if auto_count > 0 && fraction_total == 0.0 {
            remaining / auto_count as f64
        } else {
            0.0
        };

        tracks
            .iter()
            .map(|track| match track {
                TrackSize::Fixed(size) => *size,
                TrackSize::Fraction(fr) => fr * fr_unit,
                TrackSize::MinMax { min, max } => {
                    let flex_size = auto_unit;
                    flex_size.max(*min).min(*max)
                }
                TrackSize::Auto | TrackSize::MinContent | TrackSize::MaxContent => auto_unit,
            })
            .collect()
    }

    fn calculate_positions(&self, sizes: &[f64], gap: f64, start: f64) -> Vec<f64> {
        let mut positions = Vec::with_capacity(sizes.len());
        let mut current = start;

        for (i, &size) in sizes.iter().enumerate() {
            positions.push(current);
            current += size;
            if i < sizes.len() - 1 {
                current += gap;
            }
        }

        positions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_basic() {
        let layout = GridLayout::new(
            vec![TrackSize::Fixed(100.0), TrackSize::Fixed(100.0)],
            vec![TrackSize::Fixed(50.0), TrackSize::Fixed(50.0)],
        );

        let container = Bounds::new(0.0, 0.0, 200.0, 100.0);
        let children = vec![
            (GridChildSize::default(), GridPlacement::cell(1, 1)),
            (GridChildSize::default(), GridPlacement::cell(2, 1)),
            (GridChildSize::default(), GridPlacement::cell(1, 2)),
            (GridChildSize::default(), GridPlacement::cell(2, 2)),
        ];

        let result = layout.layout(container, &children);

        assert_eq!(result.len(), 4);
        assert!((result[0].x - 0.0).abs() < 0.001);
        assert!((result[0].y - 0.0).abs() < 0.001);
        assert!((result[1].x - 100.0).abs() < 0.001);
        assert!((result[1].y - 0.0).abs() < 0.001);
        assert!((result[2].x - 0.0).abs() < 0.001);
        assert!((result[2].y - 50.0).abs() < 0.001);
        assert!((result[3].x - 100.0).abs() < 0.001);
        assert!((result[3].y - 50.0).abs() < 0.001);
    }

    #[test]
    fn test_grid_with_gap() {
        let layout = GridLayout::new(
            vec![TrackSize::Fixed(90.0), TrackSize::Fixed(90.0)],
            vec![TrackSize::Fixed(40.0)],
        )
        .with_gap(20.0);

        let container = Bounds::new(0.0, 0.0, 200.0, 40.0);
        let children = vec![
            (GridChildSize::default(), GridPlacement::cell(1, 1)),
            (GridChildSize::default(), GridPlacement::cell(2, 1)),
        ];

        let result = layout.layout(container, &children);

        assert_eq!(result.len(), 2);
        assert!((result[0].x - 0.0).abs() < 0.001);
        assert!((result[1].x - 110.0).abs() < 0.001); // 90 + 20 gap
    }

    #[test]
    fn test_grid_fractional_units() {
        let layout = GridLayout::new(
            vec![TrackSize::Fraction(1.0), TrackSize::Fraction(2.0)],
            vec![TrackSize::Fixed(100.0)],
        );

        let container = Bounds::new(0.0, 0.0, 300.0, 100.0);
        let children = vec![
            (GridChildSize::default(), GridPlacement::cell(1, 1)),
            (GridChildSize::default(), GridPlacement::cell(2, 1)),
        ];

        let result = layout.layout(container, &children);

        assert_eq!(result.len(), 2);
        // First column: 1fr = 100px, second column: 2fr = 200px
        assert!((result[0].width - 100.0).abs() < 0.001);
        assert!((result[1].width - 200.0).abs() < 0.001);
    }

    #[test]
    fn test_grid_column_span() {
        let layout = GridLayout::new(
            vec![TrackSize::Fixed(100.0), TrackSize::Fixed(100.0)],
            vec![TrackSize::Fixed(50.0)],
        );

        let container = Bounds::new(0.0, 0.0, 200.0, 50.0);
        let children = vec![
            (GridChildSize::default(), GridPlacement::span_columns(1, 2, 1)),
        ];

        let result = layout.layout(container, &children);

        assert_eq!(result.len(), 1);
        // Item should span both columns
        assert!((result[0].width - 200.0).abs() < 0.001);
    }

    #[test]
    fn test_grid_alignment() {
        let layout = GridLayout::new(
            vec![TrackSize::Fixed(100.0)],
            vec![TrackSize::Fixed(100.0)],
        )
        .with_alignment(ItemAlignment::Center, ItemAlignment::Center);

        let container = Bounds::new(0.0, 0.0, 100.0, 100.0);
        let children = vec![
            (
                GridChildSize {
                    width: Some(50.0),
                    height: Some(50.0),
                    ..Default::default()
                },
                GridPlacement::cell(1, 1),
            ),
        ];

        let result = layout.layout(container, &children);

        assert_eq!(result.len(), 1);
        // Should be centered in 100x100 cell
        assert!((result[0].x - 25.0).abs() < 0.001);
        assert!((result[0].y - 25.0).abs() < 0.001);
        assert!((result[0].width - 50.0).abs() < 0.001);
        assert!((result[0].height - 50.0).abs() < 0.001);
    }

    #[test]
    fn test_grid_stretch_alignment() {
        let layout = GridLayout::new(
            vec![TrackSize::Fixed(100.0)],
            vec![TrackSize::Fixed(100.0)],
        )
        .with_alignment(ItemAlignment::Stretch, ItemAlignment::Stretch);

        let container = Bounds::new(0.0, 0.0, 100.0, 100.0);
        let children = vec![
            (
                GridChildSize {
                    width: Some(50.0),
                    height: Some(50.0),
                    ..Default::default()
                },
                GridPlacement::cell(1, 1),
            ),
        ];

        let result = layout.layout(container, &children);

        assert_eq!(result.len(), 1);
        // Should stretch to fill cell
        assert!((result[0].width - 100.0).abs() < 0.001);
        assert!((result[0].height - 100.0).abs() < 0.001);
    }

    #[test]
    fn test_grid_auto_placement() {
        let layout = GridLayout::columns(3, TrackSize::Fraction(1.0));

        let container = Bounds::new(0.0, 0.0, 300.0, 100.0);
        let children = vec![
            (GridChildSize::default(), GridPlacement::default()),
            (GridChildSize::default(), GridPlacement::default()),
            (GridChildSize::default(), GridPlacement::default()),
        ];

        let result = layout.layout(container, &children);

        assert_eq!(result.len(), 3);
        // Each column should be 100px wide (300 / 3)
        for bounds in &result {
            assert!((bounds.width - 100.0).abs() < 0.001);
        }
    }
}
