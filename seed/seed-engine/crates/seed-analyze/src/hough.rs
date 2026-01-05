//! Hough Line Transform for detecting straight lines.
//!
//! The Hough transform converts edge points into parameter space (rho, theta)
//! where lines appear as peaks. This is ideal for detecting rectangle boundaries
//! in UI screenshots.
//!
//! Key features:
//! - Detects lines regardless of gaps (robust to broken edges)
//! - Separates horizontal and vertical lines for UI element detection
//! - Returns line parameters for precise rectangle reconstruction

use std::f32::consts::PI;

/// A detected line in Hough parameter space.
#[derive(Debug, Clone, Copy)]
pub struct HoughLine {
    /// Distance from origin (perpendicular distance to line).
    pub rho: f32,
    /// Angle in radians (0 = vertical line, PI/2 = horizontal line).
    pub theta: f32,
    /// Number of votes (edge pixels on this line).
    pub votes: u32,
}

impl HoughLine {
    /// Check if this line is approximately horizontal (within tolerance).
    pub fn is_horizontal(&self, tolerance_degrees: f32) -> bool {
        let tolerance = tolerance_degrees * PI / 180.0;
        let normalized_theta = self.theta % PI;
        // Horizontal lines have theta near PI/2 or -PI/2
        (normalized_theta - PI / 2.0).abs() < tolerance
            || (normalized_theta + PI / 2.0).abs() < tolerance
    }

    /// Check if this line is approximately vertical (within tolerance).
    pub fn is_vertical(&self, tolerance_degrees: f32) -> bool {
        let tolerance = tolerance_degrees * PI / 180.0;
        let normalized_theta = self.theta % PI;
        // Vertical lines have theta near 0 or PI
        normalized_theta.abs() < tolerance || (normalized_theta - PI).abs() < tolerance
    }

    /// Convert to line segment endpoints given image dimensions.
    pub fn to_segment(&self, width: u32, height: u32) -> Option<LineSegment> {
        let cos_t = self.theta.cos();
        let sin_t = self.theta.sin();

        // Avoid division by very small numbers
        if cos_t.abs() < 1e-6 && sin_t.abs() < 1e-6 {
            return None;
        }

        let mut points = Vec::new();

        // Find intersections with image boundaries
        // Left edge (x = 0)
        if sin_t.abs() > 1e-6 {
            let y = self.rho / sin_t;
            if y >= 0.0 && y <= height as f32 {
                points.push((0.0, y));
            }
        }

        // Right edge (x = width)
        if sin_t.abs() > 1e-6 {
            let y = (self.rho - width as f32 * cos_t) / sin_t;
            if y >= 0.0 && y <= height as f32 {
                points.push((width as f32, y));
            }
        }

        // Top edge (y = 0)
        if cos_t.abs() > 1e-6 {
            let x = self.rho / cos_t;
            if x >= 0.0 && x <= width as f32 {
                points.push((x, 0.0));
            }
        }

        // Bottom edge (y = height)
        if cos_t.abs() > 1e-6 {
            let x = (self.rho - height as f32 * sin_t) / cos_t;
            if x >= 0.0 && x <= width as f32 {
                points.push((x, height as f32));
            }
        }

        // Remove duplicates and get two distinct points
        points.dedup_by(|a, b| (a.0 - b.0).abs() < 1.0 && (a.1 - b.1).abs() < 1.0);

        if points.len() >= 2 {
            Some(LineSegment {
                x1: points[0].0,
                y1: points[0].1,
                x2: points[1].0,
                y2: points[1].1,
            })
        } else {
            None
        }
    }

    /// Get the y-coordinate where this line intersects a given x.
    pub fn y_at_x(&self, x: f32) -> Option<f32> {
        let sin_t = self.theta.sin();
        if sin_t.abs() < 1e-6 {
            return None; // Horizontal line
        }
        Some((self.rho - x * self.theta.cos()) / sin_t)
    }

    /// Get the x-coordinate where this line intersects a given y.
    pub fn x_at_y(&self, y: f32) -> Option<f32> {
        let cos_t = self.theta.cos();
        if cos_t.abs() < 1e-6 {
            return None; // Vertical line
        }
        Some((self.rho - y * self.theta.sin()) / cos_t)
    }
}

/// A line segment with start and end points.
#[derive(Debug, Clone, Copy)]
pub struct LineSegment {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl LineSegment {
    /// Calculate length of segment.
    pub fn length(&self) -> f32 {
        let dx = self.x2 - self.x1;
        let dy = self.y2 - self.y1;
        (dx * dx + dy * dy).sqrt()
    }

    /// Check if segment is primarily horizontal.
    pub fn is_horizontal(&self, tolerance: f32) -> bool {
        (self.y2 - self.y1).abs() < tolerance
    }

    /// Check if segment is primarily vertical.
    pub fn is_vertical(&self, tolerance: f32) -> bool {
        (self.x2 - self.x1).abs() < tolerance
    }
}

/// Configuration for Hough transform.
#[derive(Debug, Clone)]
pub struct HoughConfig {
    /// Resolution of rho in pixels (default: 1.0).
    pub rho_resolution: f32,
    /// Resolution of theta in degrees (default: 1.0).
    pub theta_resolution_degrees: f32,
    /// Minimum votes to consider a line (default: 50).
    pub threshold: u32,
    /// Non-maximum suppression radius in accumulator space.
    pub nms_rho_radius: usize,
    pub nms_theta_radius: usize,
}

impl Default for HoughConfig {
    fn default() -> Self {
        Self {
            rho_resolution: 1.0,
            theta_resolution_degrees: 1.0,
            threshold: 50,
            nms_rho_radius: 10,
            nms_theta_radius: 5,
        }
    }
}

/// Perform Hough Line Transform on binary edge image.
///
/// Returns detected lines sorted by vote count (strongest first).
pub fn hough_lines(
    edges: &[bool],
    width: u32,
    height: u32,
    config: &HoughConfig,
) -> Vec<HoughLine> {
    let theta_res = config.theta_resolution_degrees * PI / 180.0;
    let num_thetas = (PI / theta_res).ceil() as usize;
    let max_rho = ((width * width + height * height) as f32).sqrt();
    let num_rhos = (2.0 * max_rho / config.rho_resolution).ceil() as usize;

    // Pre-compute sin/cos tables
    let thetas: Vec<f32> = (0..num_thetas).map(|i| i as f32 * theta_res).collect();
    let cos_thetas: Vec<f32> = thetas.iter().map(|t| t.cos()).collect();
    let sin_thetas: Vec<f32> = thetas.iter().map(|t| t.sin()).collect();

    // Accumulator array
    let mut accumulator = vec![0u32; num_rhos * num_thetas];

    // Vote for each edge pixel
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            if !edges[idx] {
                continue;
            }

            // Vote for all theta values
            for (t_idx, (&cos_t, &sin_t)) in cos_thetas.iter().zip(sin_thetas.iter()).enumerate() {
                let rho = x as f32 * cos_t + y as f32 * sin_t;
                let rho_idx = ((rho + max_rho) / config.rho_resolution) as usize;

                if rho_idx < num_rhos {
                    accumulator[rho_idx * num_thetas + t_idx] += 1;
                }
            }
        }
    }

    // Find peaks with non-maximum suppression
    let mut lines = Vec::new();

    for rho_idx in 0..num_rhos {
        for theta_idx in 0..num_thetas {
            let votes = accumulator[rho_idx * num_thetas + theta_idx];

            if votes < config.threshold {
                continue;
            }

            // Check if local maximum
            let mut is_max = true;
            for dr in -(config.nms_rho_radius as i32)..=(config.nms_rho_radius as i32) {
                for dt in -(config.nms_theta_radius as i32)..=(config.nms_theta_radius as i32) {
                    if dr == 0 && dt == 0 {
                        continue;
                    }

                    let nr = rho_idx as i32 + dr;
                    let nt = theta_idx as i32 + dt;

                    if nr >= 0 && nr < num_rhos as i32 && nt >= 0 && nt < num_thetas as i32 {
                        let neighbor_votes = accumulator[nr as usize * num_thetas + nt as usize];
                        if neighbor_votes > votes {
                            is_max = false;
                            break;
                        }
                    }
                }
                if !is_max {
                    break;
                }
            }

            if is_max {
                let rho = rho_idx as f32 * config.rho_resolution - max_rho;
                let theta = theta_idx as f32 * theta_res;

                lines.push(HoughLine { rho, theta, votes });
            }
        }
    }

    // Sort by vote count
    lines.sort_by(|a, b| b.votes.cmp(&a.votes));

    lines
}

/// Filter lines to get only horizontal ones.
pub fn filter_horizontal(lines: &[HoughLine], tolerance_degrees: f32) -> Vec<HoughLine> {
    lines
        .iter()
        .filter(|l| l.is_horizontal(tolerance_degrees))
        .copied()
        .collect()
}

/// Filter lines to get only vertical ones.
pub fn filter_vertical(lines: &[HoughLine], tolerance_degrees: f32) -> Vec<HoughLine> {
    lines
        .iter()
        .filter(|l| l.is_vertical(tolerance_degrees))
        .copied()
        .collect()
}

/// Merge nearby parallel lines.
///
/// Lines with similar rho and theta are merged, keeping the strongest.
pub fn merge_similar_lines(lines: &[HoughLine], rho_threshold: f32, theta_threshold: f32) -> Vec<HoughLine> {
    let mut merged = Vec::new();
    let mut used = vec![false; lines.len()];

    for (i, line) in lines.iter().enumerate() {
        if used[i] {
            continue;
        }

        used[i] = true;
        let mut group_rho = line.rho;
        let mut group_theta = line.theta;
        let mut group_votes = line.votes;
        let mut count = 1.0;

        // Find similar lines
        for (j, other) in lines.iter().enumerate().skip(i + 1) {
            if used[j] {
                continue;
            }

            let rho_diff = (line.rho - other.rho).abs();
            let theta_diff = (line.theta - other.theta).abs().min(PI - (line.theta - other.theta).abs());

            if rho_diff < rho_threshold && theta_diff < theta_threshold {
                used[j] = true;
                // Weighted average by votes
                let w = other.votes as f32;
                group_rho = (group_rho * count + other.rho * w) / (count + w);
                group_theta = (group_theta * count + other.theta * w) / (count + w);
                group_votes += other.votes;
                count += w;
            }
        }

        merged.push(HoughLine {
            rho: group_rho,
            theta: group_theta,
            votes: group_votes,
        });
    }

    merged
}

/// Find intersection point of two lines.
pub fn line_intersection(line1: &HoughLine, line2: &HoughLine) -> Option<(f32, f32)> {
    let cos1 = line1.theta.cos();
    let sin1 = line1.theta.sin();
    let cos2 = line2.theta.cos();
    let sin2 = line2.theta.sin();

    let denom = cos1 * sin2 - sin1 * cos2;

    if denom.abs() < 1e-6 {
        return None; // Parallel lines
    }

    let x = (line1.rho * sin2 - line2.rho * sin1) / denom;
    let y = (line2.rho * cos1 - line1.rho * cos2) / denom;

    Some((x, y))
}

/// Find potential rectangles from horizontal and vertical lines.
///
/// Returns bounding boxes defined by line intersections.
#[derive(Debug, Clone)]
pub struct DetectedRectangle {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// Confidence based on line strengths.
    pub confidence: f32,
}

/// Detect rectangles from sets of horizontal and vertical lines.
pub fn detect_rectangles(
    h_lines: &[HoughLine],
    v_lines: &[HoughLine],
    image_width: u32,
    image_height: u32,
    min_size: f32,
    max_size: f32,
) -> Vec<DetectedRectangle> {
    let mut rectangles = Vec::new();

    // For each pair of horizontal lines
    for (i, h1) in h_lines.iter().enumerate() {
        for h2 in h_lines.iter().skip(i + 1) {
            // For each pair of vertical lines
            for (j, v1) in v_lines.iter().enumerate() {
                for v2 in v_lines.iter().skip(j + 1) {
                    // Find 4 corners
                    let corners = [
                        line_intersection(h1, v1),
                        line_intersection(h1, v2),
                        line_intersection(h2, v1),
                        line_intersection(h2, v2),
                    ];

                    // Check all corners exist and are within image
                    let valid_corners: Vec<(f32, f32)> = corners
                        .iter()
                        .filter_map(|c| *c)
                        .filter(|(x, y)| {
                            *x >= -5.0
                                && *x <= image_width as f32 + 5.0
                                && *y >= -5.0
                                && *y <= image_height as f32 + 5.0
                        })
                        .collect();

                    if valid_corners.len() != 4 {
                        continue;
                    }

                    // Calculate bounding box
                    let min_x = valid_corners.iter().map(|c| c.0).fold(f32::MAX, f32::min);
                    let max_x = valid_corners.iter().map(|c| c.0).fold(f32::MIN, f32::max);
                    let min_y = valid_corners.iter().map(|c| c.1).fold(f32::MAX, f32::min);
                    let max_y = valid_corners.iter().map(|c| c.1).fold(f32::MIN, f32::max);

                    let width = max_x - min_x;
                    let height = max_y - min_y;

                    // Filter by size
                    if width < min_size || height < min_size {
                        continue;
                    }
                    if width > max_size || height > max_size {
                        continue;
                    }

                    // Calculate confidence from line votes
                    let total_votes = h1.votes + h2.votes + v1.votes + v2.votes;
                    let confidence = total_votes as f32 / 400.0; // Normalize

                    rectangles.push(DetectedRectangle {
                        x: min_x.max(0.0),
                        y: min_y.max(0.0),
                        width,
                        height,
                        confidence: confidence.min(1.0),
                    });
                }
            }
        }
    }

    // Sort by confidence
    rectangles.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());

    // Remove overlapping rectangles (keep highest confidence)
    filter_overlapping_rectangles(rectangles, 0.5)
}

/// Filter overlapping rectangles, keeping the highest confidence ones.
fn filter_overlapping_rectangles(
    mut rectangles: Vec<DetectedRectangle>,
    iou_threshold: f32,
) -> Vec<DetectedRectangle> {
    let mut result = Vec::new();

    while let Some(rect) = rectangles.first() {
        let rect = rect.clone();
        result.push(rect.clone());

        // Remove overlapping rectangles
        rectangles.retain(|other| {
            let iou = intersection_over_union(&rect, other);
            iou < iou_threshold
        });

        // Remove the first rectangle
        if !rectangles.is_empty() {
            rectangles.remove(0);
        }
    }

    result
}

/// Calculate intersection over union for two rectangles.
fn intersection_over_union(a: &DetectedRectangle, b: &DetectedRectangle) -> f32 {
    let x1 = a.x.max(b.x);
    let y1 = a.y.max(b.y);
    let x2 = (a.x + a.width).min(b.x + b.width);
    let y2 = (a.y + a.height).min(b.y + b.height);

    if x2 <= x1 || y2 <= y1 {
        return 0.0;
    }

    let intersection = (x2 - x1) * (y2 - y1);
    let area_a = a.width * a.height;
    let area_b = b.width * b.height;
    let union = area_a + area_b - intersection;

    if union > 0.0 {
        intersection / union
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hough_line_horizontal_check() {
        // Theta = PI/2 should be horizontal
        let line = HoughLine {
            rho: 100.0,
            theta: PI / 2.0,
            votes: 50,
        };
        assert!(line.is_horizontal(5.0));
        assert!(!line.is_vertical(5.0));
    }

    #[test]
    fn test_hough_line_vertical_check() {
        // Theta = 0 should be vertical
        let line = HoughLine {
            rho: 100.0,
            theta: 0.0,
            votes: 50,
        };
        assert!(line.is_vertical(5.0));
        assert!(!line.is_horizontal(5.0));
    }

    #[test]
    fn test_line_intersection() {
        // Vertical line at x=10 (theta=0, rho=10)
        let v_line = HoughLine {
            rho: 10.0,
            theta: 0.0,
            votes: 50,
        };
        // Horizontal line at y=20 (theta=PI/2, rho=20)
        let h_line = HoughLine {
            rho: 20.0,
            theta: PI / 2.0,
            votes: 50,
        };

        let intersection = line_intersection(&v_line, &h_line);
        assert!(intersection.is_some());

        let (x, y) = intersection.unwrap();
        assert!((x - 10.0).abs() < 0.1, "Expected x=10, got {}", x);
        assert!((y - 20.0).abs() < 0.1, "Expected y=20, got {}", y);
    }

    #[test]
    fn test_hough_detects_vertical_line() {
        // Create a binary image with a vertical line
        let width = 50u32;
        let height = 50u32;
        let mut edges = vec![false; (width * height) as usize];

        // Draw vertical line at x=25
        for y in 5..45 {
            edges[(y * width + 25) as usize] = true;
        }

        let config = HoughConfig {
            threshold: 20,
            ..Default::default()
        };

        let lines = hough_lines(&edges, width, height, &config);

        assert!(!lines.is_empty(), "Should detect at least one line");

        // Check that strongest line is vertical
        let vertical_lines = filter_vertical(&lines, 10.0);
        assert!(
            !vertical_lines.is_empty(),
            "Should detect vertical line"
        );
    }

    #[test]
    fn test_hough_detects_horizontal_line() {
        // Create a binary image with a horizontal line
        let width = 50u32;
        let height = 50u32;
        let mut edges = vec![false; (width * height) as usize];

        // Draw horizontal line at y=25
        for x in 5..45 {
            edges[(25 * width + x) as usize] = true;
        }

        let config = HoughConfig {
            threshold: 20,
            ..Default::default()
        };

        let lines = hough_lines(&edges, width, height, &config);

        assert!(!lines.is_empty(), "Should detect at least one line");

        // Check that strongest line is horizontal
        let horizontal_lines = filter_horizontal(&lines, 10.0);
        assert!(
            !horizontal_lines.is_empty(),
            "Should detect horizontal line"
        );
    }

    #[test]
    fn test_iou_calculation() {
        let rect1 = DetectedRectangle {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
            confidence: 1.0,
        };
        let rect2 = DetectedRectangle {
            x: 5.0,
            y: 5.0,
            width: 10.0,
            height: 10.0,
            confidence: 1.0,
        };

        let iou = intersection_over_union(&rect1, &rect2);
        // Intersection = 5x5 = 25, Union = 100 + 100 - 25 = 175
        let expected = 25.0 / 175.0;
        assert!(
            (iou - expected).abs() < 0.01,
            "Expected IoU {}, got {}",
            expected,
            iou
        );
    }

    #[test]
    fn test_merge_similar_lines() {
        let lines = vec![
            HoughLine { rho: 100.0, theta: 0.0, votes: 50 },
            HoughLine { rho: 102.0, theta: 0.01, votes: 40 },
            HoughLine { rho: 200.0, theta: 0.0, votes: 30 },
        ];

        let merged = merge_similar_lines(&lines, 10.0, 0.1);

        // First two should merge, third is separate
        assert_eq!(merged.len(), 2, "Should have 2 merged lines");
        assert!(merged[0].votes >= 50, "First merged line should have at least 50 votes");
    }
}
