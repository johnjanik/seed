//! Edge detection using the Sobel operator.

/// Edge detection result containing gradient magnitude at each pixel.
pub struct EdgeMap {
    pub width: u32,
    pub height: u32,
    pub magnitudes: Vec<f32>,
}

impl EdgeMap {
    /// Get magnitude at (x, y).
    pub fn get(&self, x: u32, y: u32) -> f32 {
        if x < self.width && y < self.height {
            self.magnitudes[(y * self.width + x) as usize]
        } else {
            0.0
        }
    }

    /// Check if pixel is an edge (above threshold).
    pub fn is_edge(&self, x: u32, y: u32, threshold: f32) -> bool {
        self.get(x, y) > threshold
    }
}

/// Sobel edge detection on grayscale image data.
pub fn detect_edges(pixels: &[[u8; 4]], width: u32, height: u32) -> EdgeMap {
    // Convert to grayscale
    let grayscale: Vec<f32> = pixels
        .iter()
        .map(|p| {
            // Standard luminance formula
            0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32
        })
        .collect();

    let mut magnitudes = vec![0.0f32; (width * height) as usize];

    // Sobel kernels
    // Gx: [-1, 0, 1; -2, 0, 2; -1, 0, 1]
    // Gy: [-1, -2, -1; 0, 0, 0; 1, 2, 1]

    for y in 1..height.saturating_sub(1) {
        for x in 1..width.saturating_sub(1) {
            let idx = |dx: i32, dy: i32| -> f32 {
                let nx = (x as i32 + dx) as usize;
                let ny = (y as i32 + dy) as usize;
                grayscale[ny * width as usize + nx]
            };

            // Horizontal gradient
            let gx = -idx(-1, -1) + idx(1, -1) - 2.0 * idx(-1, 0) + 2.0 * idx(1, 0) - idx(-1, 1)
                + idx(1, 1);

            // Vertical gradient
            let gy = -idx(-1, -1) - 2.0 * idx(0, -1) - idx(1, -1) + idx(-1, 1)
                + 2.0 * idx(0, 1)
                + idx(1, 1);

            // Magnitude
            let mag = (gx * gx + gy * gy).sqrt();
            magnitudes[(y * width + x) as usize] = mag;
        }
    }

    EdgeMap {
        width,
        height,
        magnitudes,
    }
}

/// Find horizontal lines from edge map.
pub fn find_horizontal_lines(edges: &EdgeMap, threshold: f32, min_length: u32) -> Vec<HLine> {
    let mut lines = Vec::new();

    for y in 0..edges.height {
        let mut x_start: Option<u32> = None;

        for x in 0..edges.width {
            let is_edge = edges.is_edge(x, y, threshold);

            match (is_edge, x_start) {
                (true, None) => {
                    x_start = Some(x);
                }
                (false, Some(start)) => {
                    let length = x - start;
                    if length >= min_length {
                        lines.push(HLine { y, x_start: start, x_end: x - 1 });
                    }
                    x_start = None;
                }
                _ => {}
            }
        }

        // Handle line extending to edge
        if let Some(start) = x_start {
            let length = edges.width - start;
            if length >= min_length {
                lines.push(HLine { y, x_start: start, x_end: edges.width - 1 });
            }
        }
    }

    lines
}

/// Find vertical lines from edge map.
pub fn find_vertical_lines(edges: &EdgeMap, threshold: f32, min_length: u32) -> Vec<VLine> {
    let mut lines = Vec::new();

    for x in 0..edges.width {
        let mut y_start: Option<u32> = None;

        for y in 0..edges.height {
            let is_edge = edges.is_edge(x, y, threshold);

            match (is_edge, y_start) {
                (true, None) => {
                    y_start = Some(y);
                }
                (false, Some(start)) => {
                    let length = y - start;
                    if length >= min_length {
                        lines.push(VLine { x, y_start: start, y_end: y - 1 });
                    }
                    y_start = None;
                }
                _ => {}
            }
        }

        // Handle line extending to edge
        if let Some(start) = y_start {
            let length = edges.height - start;
            if length >= min_length {
                lines.push(VLine { x, y_start: start, y_end: edges.height - 1 });
            }
        }
    }

    lines
}

/// A horizontal line segment.
#[derive(Debug, Clone, Copy)]
pub struct HLine {
    pub y: u32,
    pub x_start: u32,
    pub x_end: u32,
}

/// A vertical line segment.
#[derive(Debug, Clone, Copy)]
pub struct VLine {
    pub x: u32,
    pub y_start: u32,
    pub y_end: u32,
}

/// Calculate edge density in a region (for text detection).
pub fn edge_density(edges: &EdgeMap, x: u32, y: u32, width: u32, height: u32, threshold: f32) -> f32 {
    let mut edge_count = 0;
    let mut total = 0;

    for dy in 0..height {
        for dx in 0..width {
            if edges.is_edge(x + dx, y + dy, threshold) {
                edge_count += 1;
            }
            total += 1;
        }
    }

    if total > 0 {
        edge_count as f32 / total as f32
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_detection_uniform() {
        // Uniform image should have no edges
        let pixels: Vec<[u8; 4]> = vec![[128, 128, 128, 255]; 100];
        let edges = detect_edges(&pixels, 10, 10);

        // All magnitudes should be low
        assert!(edges.magnitudes.iter().all(|&m| m < 1.0));
    }

    #[test]
    fn test_edge_detection_gradient() {
        // Create a horizontal gradient
        let mut pixels = Vec::new();
        for y in 0..10 {
            for x in 0..10 {
                let v = (x * 25) as u8;
                pixels.push([v, v, v, 255]);
            }
        }

        let edges = detect_edges(&pixels, 10, 10);

        // Should detect vertical edges (horizontal gradient creates vertical edges)
        // Check middle row
        let mid_y = 5;
        let has_edges = (1..9).any(|x| edges.get(x, mid_y) > 10.0);
        assert!(has_edges);
    }
}
