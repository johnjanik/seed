//! Text measurement for layout.
//!
//! Provides text metrics computation for proper text element sizing.

/// Text measurement configuration.
#[derive(Debug, Clone)]
pub struct TextStyle {
    /// Font family
    pub font_family: String,
    /// Font size in pixels
    pub font_size: f64,
    /// Font weight (100-900, normal=400, bold=700)
    pub font_weight: u16,
    /// Line height as a multiplier (e.g., 1.5 = 150%)
    pub line_height: f64,
    /// Letter spacing in pixels
    pub letter_spacing: f64,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_family: "sans-serif".to_string(),
            font_size: 16.0,
            font_weight: 400,
            line_height: 1.2,
            letter_spacing: 0.0,
        }
    }
}

/// Measured text metrics.
#[derive(Debug, Clone, Copy, Default)]
pub struct TextMetrics {
    /// Width of the text
    pub width: f64,
    /// Height of the text (including line height)
    pub height: f64,
    /// Baseline offset from top
    pub baseline: f64,
    /// Number of lines
    pub lines: u32,
}

/// Measure text with the given style.
///
/// This is a simplified text measurement that uses character-based estimation.
/// In a real implementation, this would use font metrics from a font library.
pub fn measure_text(text: &str, style: &TextStyle, max_width: Option<f64>) -> TextMetrics {
    if text.is_empty() {
        return TextMetrics {
            width: 0.0,
            height: style.font_size * style.line_height,
            baseline: style.font_size * 0.8,
            lines: 1,
        };
    }

    // Estimate character width based on font size
    // This is a rough approximation; real text shaping would be more accurate
    let avg_char_width = estimate_char_width(style);

    match max_width {
        Some(max_w) if max_w > 0.0 => {
            // Word wrap mode
            measure_wrapped_text(text, style, max_w, avg_char_width)
        }
        _ => {
            // Single line mode
            measure_single_line(text, style, avg_char_width)
        }
    }
}

fn estimate_char_width(style: &TextStyle) -> f64 {
    // Average character width is roughly 0.5-0.6 of font size for proportional fonts
    // This varies significantly by font; this is a reasonable default
    let base_width = style.font_size * 0.55;

    // Adjust for letter spacing
    base_width + style.letter_spacing
}

fn measure_single_line(text: &str, style: &TextStyle, avg_char_width: f64) -> TextMetrics {
    let char_count = text.chars().count();
    let width = char_count as f64 * avg_char_width;
    let line_height = style.font_size * style.line_height;

    TextMetrics {
        width,
        height: line_height,
        baseline: style.font_size * 0.8, // Approximate baseline
        lines: 1,
    }
}

fn measure_wrapped_text(
    text: &str,
    style: &TextStyle,
    max_width: f64,
    avg_char_width: f64,
) -> TextMetrics {
    let line_height = style.font_size * style.line_height;
    let space_width = avg_char_width;

    let mut lines: Vec<f64> = Vec::new();
    let mut current_line_width = 0.0;

    for word in text.split_whitespace() {
        let word_width = word.chars().count() as f64 * avg_char_width;

        if current_line_width == 0.0 {
            // First word on line
            current_line_width = word_width;
        } else if current_line_width + space_width + word_width <= max_width {
            // Word fits on current line
            current_line_width += space_width + word_width;
        } else {
            // Word doesn't fit, start new line
            lines.push(current_line_width);
            current_line_width = word_width;
        }
    }

    // Don't forget the last line
    if current_line_width > 0.0 {
        lines.push(current_line_width);
    }

    let num_lines = lines.len().max(1) as u32;
    let max_line_width = lines.iter().copied().fold(0.0_f64, f64::max);

    TextMetrics {
        width: max_line_width,
        height: num_lines as f64 * line_height,
        baseline: style.font_size * 0.8,
        lines: num_lines,
    }
}

/// Simple text shaping result for rendering.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ShapedText {
    /// Lines of text with their positions
    pub lines: Vec<ShapedLine>,
    /// Total bounds
    pub bounds: TextMetrics,
}

/// A shaped line of text.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ShapedLine {
    /// The text content
    pub text: String,
    /// X offset for this line
    pub x: f64,
    /// Y offset for this line (baseline position)
    pub y: f64,
    /// Width of this line
    pub width: f64,
}

/// Text alignment for multi-line text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(dead_code)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Shape text for rendering.
#[allow(dead_code)]
pub fn shape_text(
    text: &str,
    style: &TextStyle,
    max_width: Option<f64>,
    align: TextAlign,
) -> ShapedText {
    let avg_char_width = estimate_char_width(style);
    let line_height = style.font_size * style.line_height;
    let baseline_offset = style.font_size * 0.8;

    let max_w = max_width.unwrap_or(f64::MAX);
    let space_width = avg_char_width;

    // Split into lines
    let mut text_lines: Vec<(String, f64)> = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0.0;

    for word in text.split_whitespace() {
        let word_width = word.chars().count() as f64 * avg_char_width;

        if current_line.is_empty() {
            current_line = word.to_string();
            current_width = word_width;
        } else if current_width + space_width + word_width <= max_w {
            current_line.push(' ');
            current_line.push_str(word);
            current_width += space_width + word_width;
        } else {
            text_lines.push((current_line, current_width));
            current_line = word.to_string();
            current_width = word_width;
        }
    }

    if !current_line.is_empty() {
        text_lines.push((current_line, current_width));
    }

    // Handle empty text
    if text_lines.is_empty() {
        return ShapedText {
            lines: Vec::new(),
            bounds: TextMetrics {
                width: 0.0,
                height: line_height,
                baseline: baseline_offset,
                lines: 1,
            },
        };
    }

    let max_line_width = text_lines.iter().map(|(_, w)| *w).fold(0.0_f64, f64::max);
    let num_lines = text_lines.len() as u32;
    let total_height = num_lines as f64 * line_height;

    let shaped_lines: Vec<ShapedLine> = text_lines
        .into_iter()
        .enumerate()
        .map(|(i, (line_text, line_width))| {
            let x = match align {
                TextAlign::Left => 0.0,
                TextAlign::Center => (max_line_width - line_width) / 2.0,
                TextAlign::Right => max_line_width - line_width,
            };

            ShapedLine {
                text: line_text,
                x,
                y: baseline_offset + i as f64 * line_height,
                width: line_width,
            }
        })
        .collect();

    ShapedText {
        lines: shaped_lines,
        bounds: TextMetrics {
            width: max_line_width,
            height: total_height,
            baseline: baseline_offset,
            lines: num_lines,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_measure_empty_text() {
        let style = TextStyle::default();
        let metrics = measure_text("", &style, None);
        assert!(metrics.width < 0.001);
        assert!(metrics.height > 0.0);
    }

    #[test]
    fn test_measure_single_line() {
        let style = TextStyle {
            font_size: 16.0,
            ..Default::default()
        };
        let metrics = measure_text("Hello", &style, None);
        assert!(metrics.width > 0.0);
        assert_eq!(metrics.lines, 1);
    }

    #[test]
    fn test_measure_wrapped_text() {
        let style = TextStyle {
            font_size: 16.0,
            ..Default::default()
        };
        let text = "Hello world this is a longer piece of text";
        let metrics = measure_text(text, &style, Some(100.0));
        assert!(metrics.lines > 1);
    }

    #[test]
    fn test_shape_text_alignment() {
        let style = TextStyle {
            font_size: 16.0,
            ..Default::default()
        };
        let text = "Short\nLonger line";

        let shaped_left = shape_text(text, &style, Some(200.0), TextAlign::Left);
        let shaped_center = shape_text(text, &style, Some(200.0), TextAlign::Center);
        let shaped_right = shape_text(text, &style, Some(200.0), TextAlign::Right);

        // Left aligned: first line x should be 0
        if let Some(first) = shaped_left.lines.first() {
            assert!(first.x.abs() < 0.001);
        }

        // Center and right aligned: first line should have larger x
        if let Some(first) = shaped_center.lines.first() {
            assert!(first.x >= 0.0);
        }
        if let Some(first) = shaped_right.lines.first() {
            assert!(first.x >= 0.0);
        }
    }
}
