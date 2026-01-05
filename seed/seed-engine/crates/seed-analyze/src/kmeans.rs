//! K-means++ clustering for color palette extraction.

use crate::color::Color;

/// Extract a color palette from an image using K-means++ clustering.
pub fn extract_palette(pixels: &[[u8; 4]], k: usize, max_iterations: usize) -> Vec<Color> {
    if pixels.is_empty() || k == 0 {
        return Vec::new();
    }

    // Convert pixels to Color
    let colors: Vec<Color> = pixels.iter().map(|p| Color::from_pixel(*p)).collect();

    // Sample pixels for faster clustering (use at most 10000 pixels)
    let sample_size = colors.len().min(10000);
    let step = colors.len() / sample_size;
    let sampled: Vec<&Color> = colors.iter().step_by(step.max(1)).take(sample_size).collect();

    if sampled.is_empty() {
        return Vec::new();
    }

    // K-means++ initialization
    let mut centroids = kmeans_plus_plus_init(&sampled, k);

    // Run K-means iterations
    for _ in 0..max_iterations {
        // Assign each pixel to nearest centroid
        let assignments = assign_to_centroids(&sampled, &centroids);

        // Calculate new centroids
        let new_centroids = calculate_centroids(&sampled, &assignments, k);

        // Check for convergence
        let converged = centroids
            .iter()
            .zip(new_centroids.iter())
            .all(|(old, new)| old.distance(new) < 1.0);

        centroids = new_centroids;

        if converged {
            break;
        }
    }

    // Sort by frequency (most common first)
    let mut color_counts: Vec<(Color, usize)> = centroids
        .into_iter()
        .map(|c| {
            let count = sampled.iter().filter(|p| p.distance(&c) < 20.0).count();
            (c, count)
        })
        .collect();

    color_counts.sort_by(|a, b| b.1.cmp(&a.1));
    color_counts.into_iter().map(|(c, _)| c).collect()
}

/// K-means++ initialization: select initial centroids with probability proportional to distance.
fn kmeans_plus_plus_init(pixels: &[&Color], k: usize) -> Vec<Color> {
    if pixels.is_empty() {
        return Vec::new();
    }

    let mut centroids = Vec::with_capacity(k);
    let mut rng_state: u64 = pixels.len() as u64 * 31337;

    // First centroid: random pixel
    let idx = simple_random(&mut rng_state, pixels.len());
    centroids.push(*pixels[idx]);

    // Remaining centroids: weighted by distance
    for _ in 1..k {
        // Calculate distance to nearest centroid for each pixel
        let distances: Vec<f32> = pixels
            .iter()
            .map(|p| {
                centroids
                    .iter()
                    .map(|c| p.distance(c))
                    .fold(f32::MAX, f32::min)
            })
            .collect();

        // Sum of squared distances
        let total: f32 = distances.iter().map(|d| d * d).sum();

        if total <= 0.0 {
            break;
        }

        // Pick next centroid with probability proportional to distance squared
        let threshold = simple_random_f32(&mut rng_state) * total;
        let mut cumulative = 0.0;
        let mut chosen = 0;

        for (i, d) in distances.iter().enumerate() {
            cumulative += d * d;
            if cumulative >= threshold {
                chosen = i;
                break;
            }
        }

        centroids.push(*pixels[chosen]);
    }

    centroids
}

/// Assign each pixel to its nearest centroid.
fn assign_to_centroids(pixels: &[&Color], centroids: &[Color]) -> Vec<usize> {
    pixels
        .iter()
        .map(|p| {
            centroids
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    p.distance(a)
                        .partial_cmp(&p.distance(b))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(i, _)| i)
                .unwrap_or(0)
        })
        .collect()
}

/// Calculate new centroids as mean of assigned pixels.
fn calculate_centroids(pixels: &[&Color], assignments: &[usize], k: usize) -> Vec<Color> {
    let mut sums = vec![(0u64, 0u64, 0u64, 0u64, 0usize); k];

    for (pixel, &cluster) in pixels.iter().zip(assignments.iter()) {
        if cluster < k {
            sums[cluster].0 += pixel.r as u64;
            sums[cluster].1 += pixel.g as u64;
            sums[cluster].2 += pixel.b as u64;
            sums[cluster].3 += pixel.a as u64;
            sums[cluster].4 += 1;
        }
    }

    sums.into_iter()
        .map(|(r, g, b, a, count)| {
            if count > 0 {
                Color::rgba(
                    (r / count as u64) as u8,
                    (g / count as u64) as u8,
                    (b / count as u64) as u8,
                    (a / count as u64) as u8,
                )
            } else {
                Color::rgb(128, 128, 128)
            }
        })
        .collect()
}

/// Simple pseudo-random number generator (xorshift).
fn simple_random(state: &mut u64, max: usize) -> usize {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    (*state as usize) % max
}

/// Simple random float in [0, 1).
fn simple_random_f32(state: &mut u64) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    (*state as f32) / (u64::MAX as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_palette_empty() {
        let pixels: Vec<[u8; 4]> = vec![];
        let palette = extract_palette(&pixels, 3, 10);
        assert!(palette.is_empty());
    }

    #[test]
    fn test_extract_palette_single_color() {
        let pixels: Vec<[u8; 4]> = vec![[255, 0, 0, 255]; 100];
        let palette = extract_palette(&pixels, 3, 10);
        assert!(!palette.is_empty());
        // First color should be close to red
        assert!(palette[0].r > 200);
        assert!(palette[0].g < 50);
        assert!(palette[0].b < 50);
    }

    #[test]
    fn test_extract_palette_two_colors() {
        let mut pixels: Vec<[u8; 4]> = vec![[255, 0, 0, 255]; 50];
        pixels.extend(vec![[0, 0, 255, 255]; 50]);

        let palette = extract_palette(&pixels, 2, 10);
        assert_eq!(palette.len(), 2);

        // Should find red and blue
        let has_red = palette.iter().any(|c| c.r > 200 && c.b < 50);
        let has_blue = palette.iter().any(|c| c.b > 200 && c.r < 50);
        assert!(has_red);
        assert!(has_blue);
    }
}
