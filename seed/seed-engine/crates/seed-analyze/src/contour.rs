//! Contour tracing for shape boundary extraction.
//!
//! This module implements contour tracing algorithms to find the boundaries
//! of connected regions in binary images. Contours are essential for:
//! - Shape classification (rectangle vs circle vs polygon)
//! - Precise bounding box calculation
//! - Corner detection
//! - Circularity measurement

use std::f32::consts::PI;

/// A contour representing the boundary of a region.
#[derive(Debug, Clone)]
pub struct Contour {
    /// Ordered list of boundary points (clockwise).
    pub points: Vec<(u32, u32)>,
    /// Whether the contour is closed (first == last).
    pub is_closed: bool,
    /// Pre-computed area (using shoelace formula).
    area: Option<f32>,
    /// Pre-computed perimeter.
    perimeter: Option<f32>,
}

impl Contour {
    /// Create a new contour from points.
    pub fn new(points: Vec<(u32, u32)>) -> Self {
        let is_closed = points.len() > 2 && points.first() == points.last();
        Self {
            points,
            is_closed,
            area: None,
            perimeter: None,
        }
    }

    /// Get the area enclosed by the contour (using shoelace formula).
    pub fn area(&mut self) -> f32 {
        if let Some(area) = self.area {
            return area;
        }

        if self.points.len() < 3 {
            self.area = Some(0.0);
            return 0.0;
        }

        let mut sum = 0.0f64;
        let n = self.points.len();

        for i in 0..n {
            let (x1, y1) = self.points[i];
            let (x2, y2) = self.points[(i + 1) % n];
            sum += (x1 as f64) * (y2 as f64);
            sum -= (x2 as f64) * (y1 as f64);
        }

        let area = (sum.abs() / 2.0) as f32;
        self.area = Some(area);
        area
    }

    /// Get the perimeter of the contour.
    pub fn perimeter(&mut self) -> f32 {
        if let Some(perimeter) = self.perimeter {
            return perimeter;
        }

        if self.points.len() < 2 {
            self.perimeter = Some(0.0);
            return 0.0;
        }

        let mut sum = 0.0f32;
        let n = self.points.len();

        for i in 0..n {
            let (x1, y1) = self.points[i];
            let (x2, y2) = self.points[(i + 1) % n];
            let dx = x2 as f32 - x1 as f32;
            let dy = y2 as f32 - y1 as f32;
            sum += (dx * dx + dy * dy).sqrt();
        }

        self.perimeter = Some(sum);
        sum
    }

    /// Calculate circularity: 4π × area / perimeter².
    /// Perfect circle = 1.0, rectangle ≈ 0.78, thin line ≈ 0.
    pub fn circularity(&mut self) -> f32 {
        let area = self.area();
        let perimeter = self.perimeter();

        if perimeter < 1e-6 {
            return 0.0;
        }

        4.0 * PI * area / (perimeter * perimeter)
    }

    /// Get axis-aligned bounding box.
    pub fn bounding_box(&self) -> (u32, u32, u32, u32) {
        if self.points.is_empty() {
            return (0, 0, 0, 0);
        }

        let min_x = self.points.iter().map(|p| p.0).min().unwrap_or(0);
        let max_x = self.points.iter().map(|p| p.0).max().unwrap_or(0);
        let min_y = self.points.iter().map(|p| p.1).min().unwrap_or(0);
        let max_y = self.points.iter().map(|p| p.1).max().unwrap_or(0);

        (min_x, min_y, max_x - min_x + 1, max_y - min_y + 1)
    }

    /// Get centroid of the contour.
    pub fn centroid(&self) -> (f32, f32) {
        if self.points.is_empty() {
            return (0.0, 0.0);
        }

        let sum_x: u32 = self.points.iter().map(|p| p.0).sum();
        let sum_y: u32 = self.points.iter().map(|p| p.1).sum();
        let n = self.points.len() as f32;

        (sum_x as f32 / n, sum_y as f32 / n)
    }

    /// Simplify contour using Douglas-Peucker algorithm.
    pub fn simplify(&self, epsilon: f32) -> Contour {
        if self.points.len() <= 2 {
            return self.clone();
        }

        let simplified = douglas_peucker(&self.points, epsilon);
        Contour::new(simplified)
    }

    /// Check if contour is approximately rectangular.
    /// Returns true if simplified contour has 4 corners and near 90° angles.
    pub fn is_rectangular(&self, angle_tolerance_degrees: f32) -> bool {
        let simplified = self.simplify(5.0); // Simplify first

        // A rectangle should have exactly 4 corners (5 points if closed)
        let n = if simplified.is_closed {
            simplified.points.len() - 1
        } else {
            simplified.points.len()
        };

        if n != 4 {
            return false;
        }

        // Check all angles are approximately 90 degrees
        let tolerance_rad = angle_tolerance_degrees * PI / 180.0;

        for i in 0..4 {
            let p1 = simplified.points[i];
            let p2 = simplified.points[(i + 1) % 4];
            let p3 = simplified.points[(i + 2) % 4];

            let angle = angle_between_points(p1, p2, p3);
            let diff = (angle - PI / 2.0).abs();

            if diff > tolerance_rad {
                return false;
            }
        }

        true
    }

    /// Count corners in simplified contour.
    pub fn corner_count(&self, epsilon: f32) -> usize {
        let simplified = self.simplify(epsilon);
        if simplified.is_closed && simplified.points.len() > 1 {
            simplified.points.len() - 1
        } else {
            simplified.points.len()
        }
    }
}

/// Calculate angle at point p2 formed by p1-p2-p3.
fn angle_between_points(p1: (u32, u32), p2: (u32, u32), p3: (u32, u32)) -> f32 {
    let v1 = (p1.0 as f32 - p2.0 as f32, p1.1 as f32 - p2.1 as f32);
    let v2 = (p3.0 as f32 - p2.0 as f32, p3.1 as f32 - p2.1 as f32);

    let dot = v1.0 * v2.0 + v1.1 * v2.1;
    let cross = v1.0 * v2.1 - v1.1 * v2.0;

    cross.atan2(dot).abs()
}

/// Douglas-Peucker line simplification algorithm.
fn douglas_peucker(points: &[(u32, u32)], epsilon: f32) -> Vec<(u32, u32)> {
    if points.len() <= 2 {
        return points.to_vec();
    }

    // Find the point with maximum distance from line
    let first = points[0];
    let last = points[points.len() - 1];

    let mut max_dist = 0.0f32;
    let mut max_idx = 0;

    for (i, &point) in points.iter().enumerate().skip(1).take(points.len() - 2) {
        let dist = perpendicular_distance(point, first, last);
        if dist > max_dist {
            max_dist = dist;
            max_idx = i;
        }
    }

    if max_dist > epsilon {
        // Recursively simplify
        let mut left = douglas_peucker(&points[..=max_idx], epsilon);
        let right = douglas_peucker(&points[max_idx..], epsilon);

        // Combine (remove duplicate point at junction)
        left.pop();
        left.extend(right);
        left
    } else {
        // Keep only endpoints
        vec![first, last]
    }
}

/// Calculate perpendicular distance from point to line segment.
fn perpendicular_distance(point: (u32, u32), line_start: (u32, u32), line_end: (u32, u32)) -> f32 {
    let (x, y) = (point.0 as f32, point.1 as f32);
    let (x1, y1) = (line_start.0 as f32, line_start.1 as f32);
    let (x2, y2) = (line_end.0 as f32, line_end.1 as f32);

    let dx = x2 - x1;
    let dy = y2 - y1;

    let line_len_sq = dx * dx + dy * dy;

    if line_len_sq < 1e-6 {
        // Line is a point
        return ((x - x1).powi(2) + (y - y1).powi(2)).sqrt();
    }

    // Distance from point to line
    let numerator = ((y2 - y1) * x - (x2 - x1) * y + x2 * y1 - y2 * x1).abs();
    numerator / line_len_sq.sqrt()
}

/// Direction codes for contour tracing (8-connected).
/// 0=E, 1=SE, 2=S, 3=SW, 4=W, 5=NW, 6=N, 7=NE
const DIRECTIONS: [(i32, i32); 8] = [
    (1, 0),   // 0: E
    (1, 1),   // 1: SE
    (0, 1),   // 2: S
    (-1, 1),  // 3: SW
    (-1, 0),  // 4: W
    (-1, -1), // 5: NW
    (0, -1),  // 6: N
    (1, -1),  // 7: NE
];

/// Find all contours in a binary image.
///
/// Uses the Moore boundary tracing algorithm (8-connected).
pub fn find_contours(binary: &[bool], width: u32, height: u32) -> Vec<Contour> {
    let mut contours = Vec::new();
    let mut visited = vec![false; binary.len()];

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;

            // Look for starting point: foreground pixel with background to the left
            if binary[idx] && !visited[idx] {
                let has_bg_left = x == 0 || !binary[(y * width + x - 1) as usize];

                if has_bg_left {
                    // Trace contour
                    if let Some(contour) = trace_contour(binary, width, height, x, y, &mut visited) {
                        if contour.points.len() >= 3 {
                            contours.push(contour);
                        }
                    }
                }
            }
        }
    }

    contours
}

/// Trace a single contour starting from (start_x, start_y).
fn trace_contour(
    binary: &[bool],
    width: u32,
    height: u32,
    start_x: u32,
    start_y: u32,
    visited: &mut [bool],
) -> Option<Contour> {
    let mut points = Vec::new();
    let mut x = start_x as i32;
    let mut y = start_y as i32;
    let mut dir = 0usize; // Start looking East

    // First point
    points.push((start_x, start_y));

    loop {
        visited[(y as u32 * width + x as u32) as usize] = true;

        // Find next boundary pixel by scanning counterclockwise
        let start_dir = (dir + 5) % 8; // Start from opposite + 1
        let mut found = false;

        for i in 0..8 {
            let check_dir = (start_dir + i) % 8;
            let (dx, dy) = DIRECTIONS[check_dir];
            let nx = x + dx;
            let ny = y + dy;

            if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                if binary[(ny as u32 * width + nx as u32) as usize] {
                    x = nx;
                    y = ny;
                    dir = check_dir;
                    found = true;

                    // Only add if not a duplicate of previous point
                    let new_point = (x as u32, y as u32);
                    if points.last() != Some(&new_point) {
                        points.push(new_point);
                    }

                    break;
                }
            }
        }

        if !found {
            // Isolated pixel or end of contour
            break;
        }

        // Check if we're back at the start
        if x == start_x as i32 && y == start_y as i32 {
            // Close the contour
            if points.first() != points.last() {
                points.push((start_x, start_y));
            }
            break;
        }

        // Safety: prevent infinite loops
        if points.len() > (width * height) as usize {
            break;
        }
    }

    Some(Contour::new(points))
}

/// Find only external contours (outermost boundaries).
pub fn find_external_contours(binary: &[bool], width: u32, height: u32) -> Vec<Contour> {
    let contours = find_contours(binary, width, height);

    // Filter to keep only contours not contained by others
    let mut external = Vec::new();

    for contour in &contours {
        let bbox = contour.bounding_box();
        let center = contour.centroid();

        let is_contained = contours.iter().any(|other| {
            if std::ptr::eq(contour, other) {
                return false;
            }

            let other_bbox = other.bounding_box();

            // Check if this contour's bounding box is inside other's
            bbox.0 >= other_bbox.0
                && bbox.1 >= other_bbox.1
                && bbox.0 + bbox.2 <= other_bbox.0 + other_bbox.2
                && bbox.1 + bbox.3 <= other_bbox.1 + other_bbox.3
                && point_in_contour(center, other)
        });

        if !is_contained {
            external.push(contour.clone());
        }
    }

    external
}

/// Check if a point is inside a contour (using ray casting).
pub fn point_in_contour(point: (f32, f32), contour: &Contour) -> bool {
    if contour.points.len() < 3 {
        return false;
    }

    let (px, py) = point;
    let mut inside = false;
    let n = contour.points.len();

    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = (contour.points[i].0 as f32, contour.points[i].1 as f32);
        let (xj, yj) = (contour.points[j].0 as f32, contour.points[j].1 as f32);

        if ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / (yj - yi) + xi) {
            inside = !inside;
        }

        j = i;
    }

    inside
}

/// Approximate contour with fewer points using Douglas-Peucker.
pub fn approximate_contour(contour: &Contour, epsilon: f32) -> Contour {
    contour.simplify(epsilon)
}

/// Get convex hull of contour points (Graham scan).
pub fn convex_hull(contour: &Contour) -> Contour {
    if contour.points.len() < 3 {
        return contour.clone();
    }

    // Find bottom-most point
    let mut points: Vec<(u32, u32)> = contour.points.clone();
    let pivot_idx = points
        .iter()
        .enumerate()
        .max_by_key(|(_, p)| (p.1, std::cmp::Reverse(p.0)))
        .map(|(i, _)| i)
        .unwrap_or(0);

    points.swap(0, pivot_idx);
    let pivot = points[0];

    // Sort by polar angle
    points[1..].sort_by(|a, b| {
        let angle_a = ((a.1 as f32 - pivot.1 as f32).atan2(a.0 as f32 - pivot.0 as f32) * 1000.0) as i32;
        let angle_b = ((b.1 as f32 - pivot.1 as f32).atan2(b.0 as f32 - pivot.0 as f32) * 1000.0) as i32;
        angle_a.cmp(&angle_b)
    });

    // Build hull
    let mut hull = Vec::new();

    for point in points {
        while hull.len() >= 2 && cross_product_sign(hull[hull.len() - 2], hull[hull.len() - 1], point) <= 0 {
            hull.pop();
        }
        hull.push(point);
    }

    // Close the hull
    if hull.first() != hull.last() && !hull.is_empty() {
        hull.push(hull[0]);
    }

    Contour::new(hull)
}

/// Cross product sign to determine turn direction.
fn cross_product_sign(o: (u32, u32), a: (u32, u32), b: (u32, u32)) -> i64 {
    (a.0 as i64 - o.0 as i64) * (b.1 as i64 - o.1 as i64)
        - (a.1 as i64 - o.1 as i64) * (b.0 as i64 - o.0 as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contour_area_rectangle() {
        // 10x10 rectangle
        let points = vec![
            (0, 0), (10, 0), (10, 10), (0, 10), (0, 0),
        ];
        let mut contour = Contour::new(points);

        let area = contour.area();
        assert!((area - 100.0).abs() < 0.1, "Expected area 100, got {}", area);
    }

    #[test]
    fn test_contour_perimeter_rectangle() {
        // 10x10 rectangle
        let points = vec![
            (0, 0), (10, 0), (10, 10), (0, 10), (0, 0),
        ];
        let mut contour = Contour::new(points);

        let perimeter = contour.perimeter();
        assert!((perimeter - 40.0).abs() < 0.1, "Expected perimeter 40, got {}", perimeter);
    }

    #[test]
    fn test_contour_circularity() {
        // Square has circularity ≈ 0.785 (pi/4)
        let points = vec![
            (0, 0), (10, 0), (10, 10), (0, 10), (0, 0),
        ];
        let mut contour = Contour::new(points);

        let circularity = contour.circularity();
        assert!(circularity > 0.7 && circularity < 0.85, "Expected circularity ~0.78, got {}", circularity);
    }

    #[test]
    fn test_contour_bounding_box() {
        let points = vec![
            (5, 10), (15, 10), (20, 25), (10, 30), (5, 10),
        ];
        let contour = Contour::new(points);

        let (x, y, w, h) = contour.bounding_box();
        assert_eq!(x, 5);
        assert_eq!(y, 10);
        assert_eq!(w, 16); // 20 - 5 + 1
        assert_eq!(h, 21); // 30 - 10 + 1
    }

    #[test]
    fn test_douglas_peucker_simplify() {
        // Create a noisy line that should simplify to a straight line
        let points = vec![
            (0, 0), (1, 1), (2, 0), (3, 1), (4, 0), (5, 1), (10, 0),
        ];
        let simplified = douglas_peucker(&points, 2.0);

        assert!(simplified.len() <= 3, "Should simplify to ~2-3 points, got {}", simplified.len());
        assert_eq!(simplified[0], (0, 0));
        assert_eq!(simplified.last(), Some(&(10, 0)));
    }

    #[test]
    fn test_find_contours_rectangle() {
        // Create a 6x6 image with a 4x4 filled rectangle inside
        let width = 6u32;
        let height = 6u32;
        let mut binary = vec![false; (width * height) as usize];

        // Fill rectangle from (1,1) to (4,4)
        for y in 1..5 {
            for x in 1..5 {
                binary[(y * width + x) as usize] = true;
            }
        }

        let contours = find_contours(&binary, width, height);

        assert!(!contours.is_empty(), "Should find at least one contour");

        // The contour should trace the boundary
        let contour = &contours[0];
        assert!(contour.points.len() >= 4, "Rectangle contour should have at least 4 points");
    }

    #[test]
    fn test_point_in_contour() {
        let points = vec![
            (0, 0), (10, 0), (10, 10), (0, 10), (0, 0),
        ];
        let contour = Contour::new(points);

        // Inside point
        assert!(point_in_contour((5.0, 5.0), &contour));

        // Outside point
        assert!(!point_in_contour((15.0, 5.0), &contour));

        // On edge - behavior may vary
        // assert!(point_in_contour((0.0, 5.0), &contour)); // Edge case
    }

    #[test]
    fn test_convex_hull() {
        // Points that form a star shape
        let points = vec![
            (5, 0), (6, 4), (10, 5), (6, 6), (5, 10),
            (4, 6), (0, 5), (4, 4), (5, 0),
        ];
        let contour = Contour::new(points);
        let hull = convex_hull(&contour);

        // Convex hull should have fewer points (just the outer ones)
        assert!(hull.points.len() <= 6, "Convex hull should be simpler");
    }
}
