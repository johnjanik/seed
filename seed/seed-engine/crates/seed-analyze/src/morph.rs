//! Morphological operations for cleaning up binary images.
//!
//! These operations help clean edges before shape detection:
//! - **Dilate**: Expand edges (fills small gaps)
//! - **Erode**: Shrink edges (removes noise)
//! - **Close**: Dilate then erode (fills holes while preserving size)
//! - **Open**: Erode then dilate (removes noise while preserving size)

/// Dilate a binary image (expand white regions).
///
/// Each output pixel is true if ANY pixel in its kernel neighborhood is true.
pub fn dilate(binary: &[bool], width: u32, height: u32, kernel_size: u32) -> Vec<bool> {
    let half = (kernel_size / 2) as i32;
    let mut output = vec![false; binary.len()];

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;

            // Check kernel neighborhood
            let mut any_set = false;
            'outer: for ky in -half..=half {
                for kx in -half..=half {
                    let nx = x as i32 + kx;
                    let ny = y as i32 + ky;

                    if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                        let nidx = (ny * width as i32 + nx) as usize;
                        if binary[nidx] {
                            any_set = true;
                            break 'outer;
                        }
                    }
                }
            }

            output[idx] = any_set;
        }
    }

    output
}

/// Erode a binary image (shrink white regions).
///
/// Each output pixel is true only if ALL pixels in its kernel neighborhood are true.
pub fn erode(binary: &[bool], width: u32, height: u32, kernel_size: u32) -> Vec<bool> {
    let half = (kernel_size / 2) as i32;
    let mut output = vec![false; binary.len()];

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;

            // Skip if center is already false
            if !binary[idx] {
                continue;
            }

            // Check kernel neighborhood
            let mut all_set = true;
            'outer: for ky in -half..=half {
                for kx in -half..=half {
                    let nx = x as i32 + kx;
                    let ny = y as i32 + ky;

                    if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                        let nidx = (ny * width as i32 + nx) as usize;
                        if !binary[nidx] {
                            all_set = false;
                            break 'outer;
                        }
                    } else {
                        // Out of bounds counts as false
                        all_set = false;
                        break 'outer;
                    }
                }
            }

            output[idx] = all_set;
        }
    }

    output
}

/// Close operation: dilate then erode.
///
/// Closes small gaps and holes while preserving overall shape size.
/// Good for connecting nearby edge fragments.
pub fn close(binary: &[bool], width: u32, height: u32, kernel_size: u32) -> Vec<bool> {
    let dilated = dilate(binary, width, height, kernel_size);
    erode(&dilated, width, height, kernel_size)
}

/// Open operation: erode then dilate.
///
/// Removes small noise while preserving overall shape size.
/// Good for removing isolated pixels.
pub fn open(binary: &[bool], width: u32, height: u32, kernel_size: u32) -> Vec<bool> {
    let eroded = erode(binary, width, height, kernel_size);
    dilate(&eroded, width, height, kernel_size)
}

/// Morphological gradient: dilate - erode.
///
/// Highlights boundaries of regions. Useful for finding shape outlines.
pub fn gradient(binary: &[bool], width: u32, height: u32, kernel_size: u32) -> Vec<bool> {
    let dilated = dilate(binary, width, height, kernel_size);
    let eroded = erode(binary, width, height, kernel_size);

    dilated
        .iter()
        .zip(eroded.iter())
        .map(|(&d, &e)| d && !e)
        .collect()
}

/// Fill small holes in a binary image.
///
/// Performs connected component analysis and fills regions smaller than max_hole_size.
pub fn fill_small_holes(
    binary: &[bool],
    width: u32,
    height: u32,
    max_hole_size: usize,
) -> Vec<bool> {
    let mut output = binary.to_vec();
    let mut visited = vec![false; binary.len()];

    // Find holes (connected regions of false pixels not touching border)
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let idx = (y * width + x) as usize;

            if !binary[idx] && !visited[idx] {
                // Flood fill to find connected hole
                let (hole_pixels, touches_border) =
                    flood_fill_hole(binary, width, height, x, y, &mut visited);

                // Fill if small and doesn't touch border
                if !touches_border && hole_pixels.len() <= max_hole_size {
                    for &pidx in &hole_pixels {
                        output[pidx] = true;
                    }
                }
            }
        }
    }

    output
}

/// Flood fill to find a connected region of false pixels.
fn flood_fill_hole(
    binary: &[bool],
    width: u32,
    height: u32,
    start_x: u32,
    start_y: u32,
    visited: &mut [bool],
) -> (Vec<usize>, bool) {
    let mut pixels = Vec::new();
    let mut stack = vec![(start_x, start_y)];
    let mut touches_border = false;

    while let Some((x, y)) = stack.pop() {
        let idx = (y * width + x) as usize;

        if visited[idx] || binary[idx] {
            continue;
        }

        visited[idx] = true;
        pixels.push(idx);

        // Check if touches border
        if x == 0 || x == width - 1 || y == 0 || y == height - 1 {
            touches_border = true;
        }

        // Add neighbors (4-connected)
        if x > 0 {
            stack.push((x - 1, y));
        }
        if x < width - 1 {
            stack.push((x + 1, y));
        }
        if y > 0 {
            stack.push((x, y - 1));
        }
        if y < height - 1 {
            stack.push((x, y + 1));
        }
    }

    (pixels, touches_border)
}

/// Remove small isolated regions.
///
/// Removes connected regions of true pixels smaller than min_size.
pub fn remove_small_regions(
    binary: &[bool],
    width: u32,
    height: u32,
    min_size: usize,
) -> Vec<bool> {
    let mut output = vec![false; binary.len()];
    let mut visited = vec![false; binary.len()];

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;

            if binary[idx] && !visited[idx] {
                // Flood fill to find connected region
                let region_pixels = flood_fill_region(binary, width, height, x, y, &mut visited);

                // Keep if large enough
                if region_pixels.len() >= min_size {
                    for &pidx in &region_pixels {
                        output[pidx] = true;
                    }
                }
            }
        }
    }

    output
}

/// Flood fill to find a connected region of true pixels.
fn flood_fill_region(
    binary: &[bool],
    width: u32,
    height: u32,
    start_x: u32,
    start_y: u32,
    visited: &mut [bool],
) -> Vec<usize> {
    let mut pixels = Vec::new();
    let mut stack = vec![(start_x, start_y)];

    while let Some((x, y)) = stack.pop() {
        let idx = (y * width + x) as usize;

        if visited[idx] || !binary[idx] {
            continue;
        }

        visited[idx] = true;
        pixels.push(idx);

        // Add neighbors (4-connected)
        if x > 0 {
            stack.push((x - 1, y));
        }
        if x < width - 1 {
            stack.push((x + 1, y));
        }
        if y > 0 {
            stack.push((x, y - 1));
        }
        if y < height - 1 {
            stack.push((x, y + 1));
        }
    }

    pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dilate_expands() {
        // Single pixel in center
        let mut binary = vec![false; 25];
        binary[12] = true; // Center of 5x5

        let result = dilate(&binary, 5, 5, 3);

        // Should expand to 3x3 around center
        assert!(result[6]); // Top-left of kernel
        assert!(result[7]); // Top-center
        assert!(result[8]); // Top-right
        assert!(result[11]); // Left
        assert!(result[12]); // Center
        assert!(result[13]); // Right
        assert!(result[16]); // Bottom-left
        assert!(result[17]); // Bottom-center
        assert!(result[18]); // Bottom-right
    }

    #[test]
    fn test_erode_shrinks() {
        // 3x3 block in center of 5x5
        let mut binary = vec![false; 25];
        for y in 1..4 {
            for x in 1..4 {
                binary[y * 5 + x] = true;
            }
        }

        let result = erode(&binary, 5, 5, 3);

        // Only center pixel should remain
        assert!(result[12]); // Center
        assert!(!result[6]); // Others should be gone
        assert!(!result[7]);
        assert!(!result[8]);
    }

    #[test]
    fn test_close_fills_gap() {
        // Two pixels with gap in between
        let mut binary = vec![false; 25];
        binary[11] = true; // Left
        binary[13] = true; // Right (gap of 1)

        let result = close(&binary, 5, 5, 3);

        // Gap should be filled
        assert!(result[12]); // Middle should now be filled
    }

    #[test]
    fn test_open_removes_noise() {
        // Large region with single noise pixel
        let mut binary = vec![true; 25];
        binary[0] = false; // Noise (single false in sea of true)

        // Invert for testing (we want to test removing small true regions)
        let inverted: Vec<bool> = binary.iter().map(|&b| !b).collect();

        let result = open(&inverted, 5, 5, 3);

        // Isolated pixel should be removed
        let true_count = result.iter().filter(|&&b| b).count();
        assert_eq!(true_count, 0, "Isolated pixel should be removed");
    }

    #[test]
    fn test_remove_small_regions() {
        // One large region (3x3) and one small region (1 pixel)
        let mut binary = vec![false; 100]; // 10x10

        // Large region
        for y in 2..5 {
            for x in 2..5 {
                binary[y * 10 + x] = true;
            }
        }

        // Small isolated pixel
        binary[77] = true;

        let result = remove_small_regions(&binary, 10, 10, 5);

        // Large region should remain
        assert!(result[33]); // Center of large region

        // Small region should be removed
        assert!(!result[77]);
    }
}
