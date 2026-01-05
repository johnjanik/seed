//! Image analysis and Seed code generation.
//!
//! This crate provides functionality to analyze images (UI screenshots) and
//! generate Seed markup that reproduces them.
//!
//! # Pipeline Overview
//!
//! The analysis pipeline uses computer vision techniques:
//! 1. **Preprocessing**: Scale image, convert to grayscale
//! 2. **Edge Detection**: Canny algorithm for clean, thin edges
//! 3. **Morphological Cleanup**: Close gaps, remove noise
//! 4. **Region Detection**: Color-based flood fill + edge-based refinement
//! 5. **Hierarchy Building**: Containment tree with layout analysis
//! 6. **Property Extraction**: Colors, gradients, corners, shadows
//! 7. **Code Generation**: Seed markup output
//!
//! # Example
//!
//! ```ignore
//! use seed_analyze::analyze_image;
//!
//! let png_bytes = std::fs::read("screenshot.png")?;
//! let seed_code = analyze_image(&png_bytes)?;
//! println!("{}", seed_code);
//! ```

pub mod canny;
pub mod codegen;
pub mod color;
pub mod contour;
pub mod edge;
pub mod hierarchy;
pub mod hough;
pub mod kmeans;
pub mod morph;
pub mod properties;
pub mod region;
pub mod shapes;
pub mod text;

use color::Color;
use thiserror::Error;

/// Errors that can occur during image analysis.
#[derive(Debug, Error)]
pub enum AnalyzeError {
    #[error("Failed to decode image: {0}")]
    ImageDecode(String),

    #[error("Image is too small to analyze")]
    ImageTooSmall,

    #[error("No regions detected in image")]
    NoRegions,
}

/// Configuration for image analysis.
#[derive(Debug, Clone)]
pub struct AnalyzeConfig {
    /// Maximum dimension for processing (larger images are scaled down).
    /// Increased from 400 to 800 for better detail preservation.
    pub max_dimension: u32,

    /// Color distance threshold for flood fill.
    pub color_threshold: f32,

    /// Minimum region area to consider.
    pub min_region_area: u64,

    /// Number of colors to extract for palette.
    pub palette_size: usize,

    /// Canny edge detection: low threshold for hysteresis.
    pub canny_low_threshold: f32,

    /// Canny edge detection: high threshold for hysteresis.
    pub canny_high_threshold: f32,

    /// Morphological kernel size for edge cleanup.
    pub morph_kernel_size: u32,

    /// Use enhanced edge-based detection pipeline.
    pub use_edge_detection: bool,

    /// Enable adaptive preprocessing for dark themes.
    /// When enabled, dark images get CLAHE contrast enhancement
    /// and adjusted thresholds.
    pub adaptive_dark_theme: bool,

    /// Use CLAHE (Contrast Limited Adaptive Histogram Equalization)
    /// for dark themes. Enhances subtle edges without over-amplification.
    pub use_clahe: bool,

    /// Use edge-constrained flood fill.
    /// When enabled, detected edges act as barriers that flood fill
    /// cannot cross, preventing merging of visually distinct regions.
    pub use_edge_constrained_fill: bool,

    /// Multiplier for color threshold on dark themes.
    /// Lower values (e.g., 0.5) make the threshold stricter for dark images.
    pub dark_color_threshold_mult: f32,
}

impl Default for AnalyzeConfig {
    fn default() -> Self {
        Self {
            max_dimension: 800, // Increased from 400 for better detail
            color_threshold: 15.0,
            min_region_area: 100,
            palette_size: 8,
            canny_low_threshold: 30.0,
            canny_high_threshold: 100.0,
            morph_kernel_size: 3,
            use_edge_detection: true,
            // New adaptive settings
            adaptive_dark_theme: true,
            use_clahe: true,
            use_edge_constrained_fill: true,
            dark_color_threshold_mult: 0.5,
        }
    }
}

/// Analyze a PNG image and generate Seed source code.
///
/// # Arguments
///
/// * `png_bytes` - Raw PNG file bytes
///
/// # Returns
///
/// Generated Seed markup as a string, or an error if analysis fails.
pub fn analyze_image(png_bytes: &[u8]) -> Result<String, AnalyzeError> {
    analyze_image_with_config(png_bytes, &AnalyzeConfig::default())
}

/// Analyze a PNG image with custom configuration.
pub fn analyze_image_with_config(
    png_bytes: &[u8],
    config: &AnalyzeConfig,
) -> Result<String, AnalyzeError> {
    // Decode image
    let img = image::load_from_memory(png_bytes)
        .map_err(|e| AnalyzeError::ImageDecode(e.to_string()))?;

    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    if width < 4 || height < 4 {
        return Err(AnalyzeError::ImageTooSmall);
    }

    // Scale down if too large
    let (scaled_img, scale_factor) = if width > config.max_dimension || height > config.max_dimension
    {
        let scale = config.max_dimension as f32 / width.max(height) as f32;
        let new_width = (width as f32 * scale) as u32;
        let new_height = (height as f32 * scale) as u32;

        let scaled = image::imageops::resize(
            &rgba,
            new_width,
            new_height,
            image::imageops::FilterType::Triangle,
        );
        (scaled, 1.0 / scale)
    } else {
        (rgba, 1.0)
    };

    let (proc_width, proc_height) = scaled_img.dimensions();

    // Convert to pixel array
    let pixels: Vec<[u8; 4]> = scaled_img
        .pixels()
        .map(|p| [p.0[0], p.0[1], p.0[2], p.0[3]])
        .collect();

    // Detect theme (dark vs light)
    let theme_info = canny::detect_theme(&pixels);
    let is_dark_theme = matches!(theme_info.theme, canny::ImageTheme::Dark);

    // Adjust color threshold for dark themes
    let effective_color_threshold = if is_dark_theme && config.adaptive_dark_theme {
        config.color_threshold * config.dark_color_threshold_mult
    } else {
        config.color_threshold
    };

    // Extract color palette (for future use in color quantization)
    let _palette = kmeans::extract_palette(&pixels, config.palette_size, 20);

    // Detect background color (sample corners)
    let background_color = detect_background(&pixels, proc_width, proc_height);

    // Run edge detection with adaptive preprocessing for dark themes
    let edge_result = if config.use_edge_detection {
        let use_clahe = config.use_clahe && is_dark_theme && config.adaptive_dark_theme;

        // Use adaptive Canny that handles dark themes
        let canny_result = canny::adaptive_canny_edge_detection(
            &pixels,
            proc_width,
            proc_height,
            config.canny_low_threshold,
            config.canny_high_threshold,
            use_clahe,
        );

        // Apply morphological close to connect nearby edges
        let cleaned_edges = morph::close(
            &canny_result.edges,
            proc_width,
            proc_height,
            config.morph_kernel_size,
        );

        // Remove small noise regions
        let final_edges = morph::remove_small_regions(&cleaned_edges, proc_width, proc_height, 10);

        Some(EdgeDetectionResult {
            edges: final_edges,
            magnitude: canny_result.magnitude,
            direction: canny_result.direction,
            grad_x: canny_result.grad_x,
            grad_y: canny_result.grad_y,
        })
    } else {
        None
    };

    // Detect regions - use edge-constrained fill when edges are available
    let flood_regions = if config.use_edge_constrained_fill && edge_result.is_some() {
        let edge_data = edge_result.as_ref().unwrap();
        region::detect_regions_edge_constrained(
            &pixels,
            proc_width,
            proc_height,
            &edge_data.edges,
            effective_color_threshold,
            config.min_region_area,
        )
    } else {
        region::detect_regions(
            &pixels,
            proc_width,
            proc_height,
            effective_color_threshold,
            config.min_region_area,
        )
    };

    if flood_regions.is_empty() {
        return Err(AnalyzeError::NoRegions);
    }

    // Build hierarchy from flood fill regions
    let mut regions = hierarchy::build_hierarchy(flood_regions);

    // Use edge detection results for enhanced text detection
    if let Some(edge_data) = &edge_result {
        // Create CannyResult for text detection
        let canny_result = canny::CannyResult {
            edges: edge_data.edges.clone(),
            magnitude: edge_data.magnitude.clone(),
            direction: edge_data.direction.clone(),
            grad_x: edge_data.grad_x.clone(),
            grad_y: edge_data.grad_y.clone(),
            width: proc_width,
            height: proc_height,
        };

        // Detect text regions using SWT
        let text_config = text::TextDetectConfig::default();
        let text_regions = text::detect_text_regions(
            &pixels,
            proc_width,
            proc_height,
            &canny_result,
            &text_config,
        );

        // Merge text regions into hierarchy as Text-type regions
        merge_text_regions(&mut regions, &text_regions);
    }

    // Classify regions (frame vs text) - enhanced with edge data
    hierarchy::classify_regions(&mut regions, &pixels, proc_width);

    // Analyze layout patterns (row, column, grid)
    hierarchy::analyze_layouts(&mut regions);

    // Make child positions relative
    hierarchy::make_positions_relative(&mut regions);

    // Detect properties (gradients, corner radius, stroke, shadow)
    properties::detect_all_properties(
        &mut regions,
        &pixels,
        proc_width,
        proc_height,
        &background_color,
    );

    // Scale regions back to original size
    if scale_factor != 1.0 {
        scale_regions(&mut regions, scale_factor);
    }

    // Generate Seed code
    Ok(codegen::generate_seed(&regions))
}

/// Result of edge detection for use in shape detection pipeline.
#[derive(Debug)]
pub struct EdgeDetectionResult {
    /// Binary edge map after cleanup.
    pub edges: Vec<bool>,
    /// Gradient magnitude (for Hough transform).
    pub magnitude: Vec<f32>,
    /// Gradient direction in radians (for shape orientation).
    pub direction: Vec<f32>,
    /// X gradient component (for SWT text detection).
    pub grad_x: Vec<f32>,
    /// Y gradient component (for SWT text detection).
    pub grad_y: Vec<f32>,
}

/// Detect background color by sampling corners.
fn detect_background(pixels: &[[u8; 4]], width: u32, height: u32) -> Color {
    let corners = [
        (0, 0),
        (width - 1, 0),
        (0, height - 1),
        (width - 1, height - 1),
    ];

    let mut r_sum = 0u32;
    let mut g_sum = 0u32;
    let mut b_sum = 0u32;

    for (x, y) in corners {
        let idx = (y * width + x) as usize;
        if let Some(p) = pixels.get(idx) {
            r_sum += p[0] as u32;
            g_sum += p[1] as u32;
            b_sum += p[2] as u32;
        }
    }

    Color::rgb(
        (r_sum / 4) as u8,
        (g_sum / 4) as u8,
        (b_sum / 4) as u8,
    )
}

/// Merge detected text regions into the region hierarchy.
fn merge_text_regions(regions: &mut Vec<region::Region>, text_regions: &[text::TextRegion]) {
    for text_reg in text_regions {
        // Skip low confidence text detections
        if text_reg.confidence < 0.3 {
            continue;
        }

        // Create a new Region for this text
        let mut new_region = region::Region::new(
            text_reg.bounds,
            region::Fill::Solid(text_reg.text_color.clone()),
        );
        new_region.region_type = region::RegionType::Text;

        // Find the best parent region (smallest containing region)
        let mut inserted = false;
        for parent in regions.iter_mut() {
            if try_insert_text_region(parent, &new_region) {
                inserted = true;
                break;
            }
        }

        // If no parent found, add as root-level region
        if !inserted {
            regions.push(new_region);
        }
    }
}

/// Try to insert a text region as a child of an appropriate parent.
/// Returns true if inserted.
fn try_insert_text_region(parent: &mut region::Region, text_region: &region::Region) -> bool {
    let parent_bounds = &parent.bounds;
    let text_bounds = &text_region.bounds;

    // Check if parent contains the text region
    if !parent_bounds.contains_bounds(text_bounds) {
        return false;
    }

    // Try to insert into a child first (find smallest container)
    for child in &mut parent.children {
        if try_insert_text_region(child, text_region) {
            return true;
        }
    }

    // Check if this text region overlaps with existing children
    // If it does, don't add it (already detected via flood fill)
    for child in &parent.children {
        if child.bounds.overlaps(text_bounds) {
            // If existing child is not text, mark it as text
            // (don't create duplicate)
            return true;
        }
    }

    // Insert as child of this parent
    parent.children.push(text_region.clone());
    true
}

/// Scale region bounds back to original image size.
fn scale_regions(regions: &mut [region::Region], factor: f32) {
    for region in regions {
        region.bounds.x = (region.bounds.x as f32 * factor).round() as u32;
        region.bounds.y = (region.bounds.y as f32 * factor).round() as u32;
        region.bounds.width = (region.bounds.width as f32 * factor).round() as u32;
        region.bounds.height = (region.bounds.height as f32 * factor).round() as u32;
        region.corner_radius *= factor;

        if let Some(stroke) = &mut region.stroke {
            stroke.width *= factor;
        }

        if let Some(shadow) = &mut region.shadow {
            shadow.offset_x *= factor;
            shadow.offset_y *= factor;
            shadow.blur *= factor;
        }

        scale_regions(&mut region.children, factor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_config_default() {
        let config = AnalyzeConfig::default();
        assert_eq!(config.max_dimension, 800); // Increased for better detail
        assert_eq!(config.palette_size, 8);
        assert_eq!(config.canny_low_threshold, 30.0);
        assert_eq!(config.canny_high_threshold, 100.0);
        assert_eq!(config.morph_kernel_size, 3);
        assert!(config.use_edge_detection);
        // New adaptive settings
        assert!(config.adaptive_dark_theme);
        assert!(config.use_clahe);
        assert!(config.use_edge_constrained_fill);
        assert_eq!(config.dark_color_threshold_mult, 0.5);
    }

    #[test]
    fn test_edge_detection_result_struct() {
        // Verify EdgeDetectionResult can hold all necessary data
        let result = EdgeDetectionResult {
            edges: vec![true, false, true],
            magnitude: vec![1.0, 2.0, 3.0],
            direction: vec![0.0, 1.57, 3.14],
            grad_x: vec![1.0, 0.0, -1.0],
            grad_y: vec![0.0, 1.0, 0.0],
        };
        assert_eq!(result.edges.len(), 3);
        assert_eq!(result.magnitude.len(), 3);
    }

    // Note: Full integration tests would require actual PNG test data
}
