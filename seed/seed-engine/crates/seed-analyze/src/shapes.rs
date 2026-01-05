//! Shape classification and fitting for detected contours.
//!
//! This module classifies contours into geometric primitives:
//! - Rectangles (with optional rounded corners)
//! - Ellipses/circles
//! - Lines/dividers
//! - General polygons
//!
//! These classifications map directly to Seed element types.

use crate::contour::Contour;
use std::f32::consts::PI;

/// Detected shape classification with fitted parameters.
#[derive(Debug, Clone)]
pub enum DetectedShape {
    /// Axis-aligned or rotated rectangle.
    Rectangle {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        /// Per-corner radii: [top_left, top_right, bottom_right, bottom_left]
        corner_radii: [f32; 4],
        /// Rotation angle in radians (0 = axis-aligned).
        rotation: f32,
        /// Fit quality (0.0-1.0, higher is better).
        confidence: f32,
    },

    /// Ellipse or circle.
    Ellipse {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        rotation: f32,
        confidence: f32,
    },

    /// Thin line/divider.
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        thickness: f32,
        confidence: f32,
    },

    /// General polygon (fallback).
    Polygon {
        points: Vec<(f32, f32)>,
        confidence: f32,
    },
}

impl DetectedShape {
    /// Get bounding box of the shape.
    pub fn bounding_box(&self) -> (f32, f32, f32, f32) {
        match self {
            DetectedShape::Rectangle { x, y, width, height, .. } => (*x, *y, *width, *height),
            DetectedShape::Ellipse { cx, cy, rx, ry, .. } => {
                (cx - rx, cy - ry, rx * 2.0, ry * 2.0)
            }
            DetectedShape::Line { x1, y1, x2, y2, thickness, .. } => {
                let min_x = x1.min(*x2) - thickness / 2.0;
                let min_y = y1.min(*y2) - thickness / 2.0;
                let max_x = x1.max(*x2) + thickness / 2.0;
                let max_y = y1.max(*y2) + thickness / 2.0;
                (min_x, min_y, max_x - min_x, max_y - min_y)
            }
            DetectedShape::Polygon { points, .. } => {
                if points.is_empty() {
                    return (0.0, 0.0, 0.0, 0.0);
                }
                let min_x = points.iter().map(|p| p.0).fold(f32::MAX, f32::min);
                let max_x = points.iter().map(|p| p.0).fold(f32::MIN, f32::max);
                let min_y = points.iter().map(|p| p.1).fold(f32::MAX, f32::min);
                let max_y = points.iter().map(|p| p.1).fold(f32::MIN, f32::max);
                (min_x, min_y, max_x - min_x, max_y - min_y)
            }
        }
    }

    /// Get confidence score.
    pub fn confidence(&self) -> f32 {
        match self {
            DetectedShape::Rectangle { confidence, .. } => *confidence,
            DetectedShape::Ellipse { confidence, .. } => *confidence,
            DetectedShape::Line { confidence, .. } => *confidence,
            DetectedShape::Polygon { confidence, .. } => *confidence,
        }
    }

    /// Check if this is a circle (ellipse with equal radii).
    pub fn is_circle(&self) -> bool {
        match self {
            DetectedShape::Ellipse { rx, ry, .. } => (rx - ry).abs() < rx.max(*ry) * 0.1,
            _ => false,
        }
    }

    /// Check if this is a horizontal line.
    pub fn is_horizontal_line(&self) -> bool {
        match self {
            DetectedShape::Line { y1, y2, .. } => (y2 - y1).abs() < 3.0,
            _ => false,
        }
    }

    /// Check if this is a vertical line.
    pub fn is_vertical_line(&self) -> bool {
        match self {
            DetectedShape::Line { x1, x2, .. } => (x2 - x1).abs() < 3.0,
            _ => false,
        }
    }
}

/// Configuration for shape fitting.
#[derive(Debug, Clone)]
pub struct ShapeFitConfig {
    /// Circularity threshold for ellipse detection (0.0-1.0).
    pub circularity_threshold: f32,
    /// Aspect ratio threshold for line detection.
    pub line_aspect_ratio: f32,
    /// Minimum fit quality to accept a rectangle fit.
    pub rect_min_confidence: f32,
    /// Douglas-Peucker epsilon for polygon simplification.
    pub polygon_epsilon: f32,
    /// Tolerance for right angles in rectangle detection (degrees).
    pub right_angle_tolerance: f32,
}

impl Default for ShapeFitConfig {
    fn default() -> Self {
        Self {
            circularity_threshold: 0.75,
            line_aspect_ratio: 10.0,
            rect_min_confidence: 0.7,
            polygon_epsilon: 3.0,
            right_angle_tolerance: 15.0,
        }
    }
}

/// Classify and fit a contour to the best matching shape.
pub fn classify_shape(contour: &mut Contour, config: &ShapeFitConfig) -> DetectedShape {
    // Get basic properties
    let circularity = contour.circularity();
    let (_x, _y, w, h) = contour.bounding_box();
    let aspect_ratio = w.max(1) as f32 / h.max(1) as f32;

    // Check for line (very elongated)
    if aspect_ratio > config.line_aspect_ratio || aspect_ratio < 1.0 / config.line_aspect_ratio {
        if let Some(line) = try_fit_line(contour) {
            return line;
        }
    }

    // Check for circle/ellipse (high circularity)
    if circularity > config.circularity_threshold {
        return fit_ellipse(contour);
    }

    // Try rectangle fit
    if let Some(rect) = try_fit_rectangle(contour, config) {
        if rect.confidence() >= config.rect_min_confidence {
            return rect;
        }
    }

    // Fallback to polygon
    fit_polygon(contour, config.polygon_epsilon)
}

/// Try to fit a line to a very elongated contour.
fn try_fit_line(contour: &Contour) -> Option<DetectedShape> {
    let (x, y, w, h) = contour.bounding_box();

    // Determine if horizontal or vertical
    let is_horizontal = w > h;
    let thickness = if is_horizontal { h } else { w } as f32;

    // Lines should be thin
    if thickness > 10.0 {
        return None;
    }

    let (x1, y1, x2, y2) = if is_horizontal {
        (
            x as f32,
            (y as f32 + h as f32 / 2.0),
            (x + w) as f32,
            (y as f32 + h as f32 / 2.0),
        )
    } else {
        (
            (x as f32 + w as f32 / 2.0),
            y as f32,
            (x as f32 + w as f32 / 2.0),
            (y + h) as f32,
        )
    };

    Some(DetectedShape::Line {
        x1,
        y1,
        x2,
        y2,
        thickness: thickness.max(1.0),
        confidence: 0.9,
    })
}

/// Fit an ellipse to the contour.
fn fit_ellipse(contour: &Contour) -> DetectedShape {
    let (x, y, w, h) = contour.bounding_box();

    let cx = x as f32 + w as f32 / 2.0;
    let cy = y as f32 + h as f32 / 2.0;
    let rx = w as f32 / 2.0;
    let ry = h as f32 / 2.0;

    // Estimate confidence from circularity
    let mut contour_clone = contour.clone();
    let circularity = contour_clone.circularity();
    let confidence = circularity.min(1.0);

    DetectedShape::Ellipse {
        cx,
        cy,
        rx,
        ry,
        rotation: 0.0, // TODO: compute actual rotation for non-axis-aligned ellipses
        confidence,
    }
}

/// Try to fit a rectangle to the contour.
fn try_fit_rectangle(contour: &Contour, config: &ShapeFitConfig) -> Option<DetectedShape> {
    // Simplify contour to find corners
    let simplified = contour.simplify(config.polygon_epsilon);
    let corner_count = if simplified.is_closed && simplified.points.len() > 1 {
        simplified.points.len() - 1
    } else {
        simplified.points.len()
    };

    // A rectangle should have 4 corners
    if corner_count != 4 {
        return None;
    }

    // Get the 4 corner points
    let corners: Vec<(f32, f32)> = simplified.points[..4]
        .iter()
        .map(|&(x, y)| (x as f32, y as f32))
        .collect();

    // Check if angles are approximately 90 degrees
    let tolerance_rad = config.right_angle_tolerance * PI / 180.0;
    let mut angle_quality = 0.0f32;

    for i in 0..4 {
        let p1 = corners[i];
        let p2 = corners[(i + 1) % 4];
        let p3 = corners[(i + 2) % 4];

        let angle = angle_at_point(p1, p2, p3);
        let diff = (angle - PI / 2.0).abs();

        if diff > tolerance_rad {
            return None; // Not a right angle
        }

        angle_quality += 1.0 - (diff / tolerance_rad);
    }

    angle_quality /= 4.0;

    // Calculate bounding rectangle
    let min_x = corners.iter().map(|c| c.0).fold(f32::MAX, f32::min);
    let max_x = corners.iter().map(|c| c.0).fold(f32::MIN, f32::max);
    let min_y = corners.iter().map(|c| c.1).fold(f32::MAX, f32::min);
    let max_y = corners.iter().map(|c| c.1).fold(f32::MIN, f32::max);

    // Detect corner radii by analyzing the original contour
    let corner_radii = detect_corner_radii(contour, (min_x, min_y, max_x, max_y));

    Some(DetectedShape::Rectangle {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
        corner_radii,
        rotation: 0.0, // TODO: detect rotation
        confidence: angle_quality,
    })
}

/// Calculate angle at point p2 formed by p1-p2-p3.
fn angle_at_point(p1: (f32, f32), p2: (f32, f32), p3: (f32, f32)) -> f32 {
    let v1 = (p1.0 - p2.0, p1.1 - p2.1);
    let v2 = (p3.0 - p2.0, p3.1 - p2.1);

    let dot = v1.0 * v2.0 + v1.1 * v2.1;
    let cross = v1.0 * v2.1 - v1.1 * v2.0;

    cross.atan2(dot).abs()
}

/// Detect corner radii by analyzing how the contour deviates from sharp corners.
fn detect_corner_radii(contour: &Contour, bbox: (f32, f32, f32, f32)) -> [f32; 4] {
    let (min_x, min_y, max_x, max_y) = bbox;

    // Define corner regions
    let corners = [
        (min_x, min_y), // top-left
        (max_x, min_y), // top-right
        (max_x, max_y), // bottom-right
        (min_x, max_y), // bottom-left
    ];

    let search_radius = ((max_x - min_x).min(max_y - min_y) / 4.0).max(5.0);

    let mut radii = [0.0f32; 4];

    for (i, &(cx, cy)) in corners.iter().enumerate() {
        // Find contour points near this corner
        let nearby_points: Vec<(f32, f32)> = contour
            .points
            .iter()
            .map(|&(x, y)| (x as f32, y as f32))
            .filter(|&(x, y)| {
                let dx = x - cx;
                let dy = y - cy;
                (dx * dx + dy * dy).sqrt() < search_radius
            })
            .collect();

        if nearby_points.len() < 3 {
            continue;
        }

        // Estimate radius from how far points deviate from corner
        let max_deviation = nearby_points
            .iter()
            .map(|&(x, y)| {
                let dx = (x - cx).abs();
                let dy = (y - cy).abs();
                dx.min(dy)
            })
            .fold(0.0f32, f32::max);

        radii[i] = max_deviation * 1.5; // Approximate radius
    }

    radii
}

/// Fit a polygon to the contour.
fn fit_polygon(contour: &Contour, epsilon: f32) -> DetectedShape {
    let simplified = contour.simplify(epsilon);

    let points: Vec<(f32, f32)> = simplified
        .points
        .iter()
        .map(|&(x, y)| (x as f32, y as f32))
        .collect();

    // Confidence based on simplification ratio
    let original_count = contour.points.len() as f32;
    let simplified_count = points.len() as f32;
    let confidence = (simplified_count / original_count).min(1.0);

    DetectedShape::Polygon { points, confidence }
}

/// Batch classify multiple contours.
pub fn classify_contours(contours: &mut [Contour], config: &ShapeFitConfig) -> Vec<DetectedShape> {
    contours
        .iter_mut()
        .map(|c| classify_shape(c, config))
        .collect()
}

/// Filter shapes by minimum area.
pub fn filter_by_area(shapes: Vec<DetectedShape>, min_area: f32) -> Vec<DetectedShape> {
    shapes
        .into_iter()
        .filter(|s| {
            let (_, _, w, h) = s.bounding_box();
            w * h >= min_area
        })
        .collect()
}

/// Merge nearby similar rectangles.
pub fn merge_overlapping_rectangles(shapes: Vec<DetectedShape>, iou_threshold: f32) -> Vec<DetectedShape> {
    let mut result = Vec::new();
    let mut used = vec![false; shapes.len()];

    for (i, shape) in shapes.iter().enumerate() {
        if used[i] {
            continue;
        }

        if let DetectedShape::Rectangle { x, y, width, height, corner_radii, rotation, confidence } = shape {
            used[i] = true;

            let mut merged_x = *x;
            let mut merged_y = *y;
            let mut merged_w = *width;
            let mut merged_h = *height;
            let mut merged_radii = *corner_radii;
            let mut merged_confidence = *confidence;
            let mut count = 1.0;

            // Find overlapping rectangles
            for (j, other) in shapes.iter().enumerate().skip(i + 1) {
                if used[j] {
                    continue;
                }

                if let DetectedShape::Rectangle {
                    x: ox,
                    y: oy,
                    width: ow,
                    height: oh,
                    corner_radii: or,
                    confidence: oc,
                    ..
                } = other
                {
                    let iou = calculate_iou(
                        (merged_x, merged_y, merged_w, merged_h),
                        (*ox, *oy, *ow, *oh),
                    );

                    if iou > iou_threshold {
                        used[j] = true;

                        // Weighted merge
                        let w = *oc;
                        merged_x = (merged_x * count + ox * w) / (count + w);
                        merged_y = (merged_y * count + oy * w) / (count + w);
                        merged_w = (merged_w * count + ow * w) / (count + w);
                        merged_h = (merged_h * count + oh * w) / (count + w);

                        for k in 0..4 {
                            merged_radii[k] = (merged_radii[k] * count + or[k] * w) / (count + w);
                        }

                        merged_confidence = merged_confidence.max(*oc);
                        count += w;
                    }
                }
            }

            result.push(DetectedShape::Rectangle {
                x: merged_x,
                y: merged_y,
                width: merged_w,
                height: merged_h,
                corner_radii: merged_radii,
                rotation: *rotation,
                confidence: merged_confidence,
            });
        } else {
            result.push(shape.clone());
        }
    }

    result
}

/// Calculate intersection over union for two rectangles.
fn calculate_iou(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> f32 {
    let x1 = a.0.max(b.0);
    let y1 = a.1.max(b.1);
    let x2 = (a.0 + a.2).min(b.0 + b.2);
    let y2 = (a.1 + a.3).min(b.1 + b.3);

    if x2 <= x1 || y2 <= y1 {
        return 0.0;
    }

    let intersection = (x2 - x1) * (y2 - y1);
    let area_a = a.2 * a.3;
    let area_b = b.2 * b.3;
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

    fn make_rectangle_contour(x: u32, y: u32, w: u32, h: u32) -> Contour {
        Contour::new(vec![
            (x, y),
            (x + w, y),
            (x + w, y + h),
            (x, y + h),
            (x, y),
        ])
    }

    fn make_circle_contour(cx: u32, cy: u32, r: u32) -> Contour {
        let n = 32;
        let mut points = Vec::with_capacity(n + 1);

        for i in 0..=n {
            let angle = 2.0 * PI * i as f32 / n as f32;
            let x = (cx as f32 + r as f32 * angle.cos()).round() as u32;
            let y = (cy as f32 + r as f32 * angle.sin()).round() as u32;
            points.push((x, y));
        }

        Contour::new(points)
    }

    #[test]
    fn test_classify_rectangle() {
        let mut contour = make_rectangle_contour(10, 20, 100, 50);
        let config = ShapeFitConfig::default();

        let shape = classify_shape(&mut contour, &config);

        match shape {
            DetectedShape::Rectangle { x, y, width, height, .. } => {
                assert!((x - 10.0).abs() < 1.0);
                assert!((y - 20.0).abs() < 1.0);
                assert!((width - 100.0).abs() < 1.0);
                assert!((height - 50.0).abs() < 1.0);
            }
            _ => panic!("Expected rectangle, got {:?}", shape),
        }
    }

    #[test]
    fn test_classify_circle() {
        let mut contour = make_circle_contour(50, 50, 20);
        let config = ShapeFitConfig::default();

        let shape = classify_shape(&mut contour, &config);

        match shape {
            DetectedShape::Ellipse { cx, cy, rx, ry, .. } => {
                assert!((cx - 50.0).abs() < 2.0, "cx={}", cx);
                assert!((cy - 50.0).abs() < 2.0, "cy={}", cy);
                assert!((rx - 20.0).abs() < 2.0, "rx={}", rx);
                assert!((ry - 20.0).abs() < 2.0, "ry={}", ry);
            }
            _ => panic!("Expected ellipse, got {:?}", shape),
        }
    }

    #[test]
    fn test_classify_horizontal_line() {
        // Very thin horizontal rectangle
        let mut contour = make_rectangle_contour(10, 50, 200, 2);
        let config = ShapeFitConfig::default();

        let shape = classify_shape(&mut contour, &config);

        match shape {
            DetectedShape::Line { x1, x2, thickness, .. } => {
                assert!((x2 - x1 - 200.0).abs() < 5.0);
                assert!(thickness < 5.0);
            }
            _ => panic!("Expected line, got {:?}", shape),
        }
    }

    #[test]
    fn test_is_circle() {
        let ellipse = DetectedShape::Ellipse {
            cx: 50.0,
            cy: 50.0,
            rx: 20.0,
            ry: 20.0,
            rotation: 0.0,
            confidence: 1.0,
        };
        assert!(ellipse.is_circle());

        let non_circle = DetectedShape::Ellipse {
            cx: 50.0,
            cy: 50.0,
            rx: 30.0,
            ry: 20.0,
            rotation: 0.0,
            confidence: 1.0,
        };
        assert!(!non_circle.is_circle());
    }

    #[test]
    fn test_iou_calculation() {
        let a = (0.0, 0.0, 10.0, 10.0);
        let b = (5.0, 5.0, 10.0, 10.0);

        let iou = calculate_iou(a, b);
        // Intersection = 5x5 = 25, Union = 100 + 100 - 25 = 175
        let expected = 25.0 / 175.0;
        assert!((iou - expected).abs() < 0.01);
    }

    #[test]
    fn test_filter_by_area() {
        let shapes = vec![
            DetectedShape::Rectangle {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
                corner_radii: [0.0; 4],
                rotation: 0.0,
                confidence: 1.0,
            },
            DetectedShape::Rectangle {
                x: 0.0,
                y: 0.0,
                width: 5.0,
                height: 5.0,
                corner_radii: [0.0; 4],
                rotation: 0.0,
                confidence: 1.0,
            },
        ];

        let filtered = filter_by_area(shapes, 50.0);
        assert_eq!(filtered.len(), 1);
    }
}
