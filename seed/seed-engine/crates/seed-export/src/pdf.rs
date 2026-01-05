//! PDF export.
//!
//! This module exports Seed documents to PDF format using pdf-writer.

use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str};
use seed_core::{
    ast::{Element, FrameElement, TextElement, Property, PropertyValue, TextContent},
    types::Color,
    Document, ExportError,
};
use seed_layout::{LayoutTree, LayoutNodeId};
use seed_render_2d::primitives::CornerRadius;

/// Export a document to PDF.
pub fn export(doc: &Document, layout: &LayoutTree) -> Result<Vec<u8>, ExportError> {
    let options = PdfOptions::default();
    export_with_options(doc, layout, &options)
}

/// PDF export options.
pub struct PdfOptions {
    /// Page width in points (1/72 inch)
    pub width: f32,
    /// Page height in points (1/72 inch)
    pub height: f32,
    /// Whether to compress streams
    pub compress: bool,
}

impl Default for PdfOptions {
    fn default() -> Self {
        // Get size from layout or use A4
        Self {
            width: 595.0,  // A4 width in points
            height: 842.0, // A4 height in points
            compress: true,
        }
    }
}

/// Export a document to PDF with options.
pub fn export_with_options(
    doc: &Document,
    layout: &LayoutTree,
    options: &PdfOptions,
) -> Result<Vec<u8>, ExportError> {
    // Determine page size from layout bounds if available
    let bounds = layout.content_bounds();
    let (page_width, page_height) = if bounds.width > 0.0 && bounds.height > 0.0 {
        (bounds.width as f32, bounds.height as f32)
    } else {
        (options.width, options.height)
    };

    let mut pdf = Pdf::new();

    // Allocate object references
    let catalog_id = Ref::new(1);
    let page_tree_id = Ref::new(2);
    let page_id = Ref::new(3);
    let content_id = Ref::new(4);
    let font_id = Ref::new(5);

    // Write catalog
    pdf.catalog(catalog_id).pages(page_tree_id);

    // Write page tree
    pdf.pages(page_tree_id).kids([page_id]).count(1);

    // Build content stream
    let mut content = Content::new();

    // Set up coordinate system (PDF has origin at bottom-left, we use top-left)
    // Translate to top-left and flip Y
    content.transform([1.0, 0.0, 0.0, -1.0, 0.0, page_height]);

    // Build PDF content from document
    let mut builder = PdfBuilder {
        content: &mut content,
        layout,
    };

    for &root_id in layout.roots() {
        if layout.get(root_id).is_some() {
            for element in &doc.elements {
                builder.render_element(element, root_id);
            }
        }
    }

    let content_data = content.finish();

    // Write page
    let mut page = pdf.page(page_id);
    page.media_box(Rect::new(0.0, 0.0, page_width, page_height));
    page.parent(page_tree_id);
    page.contents(content_id);

    // Add font resource
    let mut resources = page.resources();
    resources.fonts().pair(Name(b"F1"), font_id);
    resources.finish();
    page.finish();

    // Write content stream
    pdf.stream(content_id, &content_data);

    // Write font (use built-in Helvetica)
    pdf.type1_font(font_id).base_font(Name(b"Helvetica"));

    Ok(pdf.finish())
}

struct PdfBuilder<'a> {
    content: &'a mut Content,
    layout: &'a LayoutTree,
}

impl<'a> PdfBuilder<'a> {
    fn render_element(&mut self, element: &Element, node_id: LayoutNodeId) {
        match element {
            Element::Frame(frame) => self.render_frame(frame, node_id),
            Element::Text(text) => self.render_text(text, node_id),
            Element::Svg(svg) => self.render_svg(svg, node_id),
            Element::Image(image) => self.render_image(image, node_id),
            Element::Icon(icon) => self.render_icon(icon, node_id),
            Element::Part(_) => {
                // 3D parts don't render in PDF
            }
            Element::Component(_) | Element::Slot(_) => {
                // Should be expanded before rendering
            }
        }
    }

    fn render_frame(&mut self, frame: &FrameElement, node_id: LayoutNodeId) {
        let Some(node) = self.layout.get(node_id) else {
            return;
        };

        if !node.visible || node.opacity <= 0.0 {
            return;
        }

        let bounds = node.absolute_bounds;
        let x = bounds.x as f32;
        let y = bounds.y as f32;
        let w = bounds.width as f32;
        let h = bounds.height as f32;

        // Get fill color
        let fill_color = get_fill_color(&frame.properties);
        let stroke_color = get_stroke_color(&frame.properties);
        let stroke_width = get_stroke_width(&frame.properties);
        let corner_radius = get_corner_radius(&frame.properties);

        // Draw background/fill
        if let Some(color) = fill_color {
            self.content.set_fill_rgb(color.r, color.g, color.b);

            if is_rounded(&corner_radius) {
                self.draw_rounded_rect(x, y, w, h, &corner_radius);
                self.content.fill_nonzero();
            } else {
                self.content.rect(x, y, w, h);
                self.content.fill_nonzero();
            }
        }

        // Draw stroke/border
        if let Some(color) = stroke_color {
            self.content.set_stroke_rgb(color.r, color.g, color.b);
            self.content.set_line_width(stroke_width);

            if is_rounded(&corner_radius) {
                self.draw_rounded_rect(x, y, w, h, &corner_radius);
                self.content.stroke();
            } else {
                self.content.rect(x, y, w, h);
                self.content.stroke();
            }
        }

        // Render children
        for &child_id in &node.children {
            for child in &frame.children {
                self.render_element(child, child_id);
            }
        }
    }

    fn render_text(&mut self, text: &TextElement, node_id: LayoutNodeId) {
        let Some(node) = self.layout.get(node_id) else {
            return;
        };

        if !node.visible || node.opacity <= 0.0 {
            return;
        }

        let bounds = node.absolute_bounds;
        let x = bounds.x as f32;
        let y = bounds.y as f32;

        // Get text content
        let content_str = match &text.content {
            TextContent::Literal(s) => s.clone(),
            TextContent::TokenRef(_) => "[token]".to_string(),
        };

        // Get text properties
        let color = get_color_property(&text.properties, "color")
            .unwrap_or(Color::BLACK);
        let font_size = get_length_property(&text.properties, "font-size")
            .unwrap_or(12.0) as f32;

        // Draw text
        self.content.set_fill_rgb(color.r, color.g, color.b);
        self.content.begin_text();
        self.content.set_font(Name(b"F1"), font_size);
        // Position text (add baseline offset - approximate)
        self.content.next_line(x, y + font_size * 0.8);
        self.content.show(Str(content_str.as_bytes()));
        self.content.end_text();
    }

    fn render_svg(&mut self, svg: &seed_core::ast::SvgElement, node_id: LayoutNodeId) {
        use seed_core::ast::SvgPathCommand;

        let Some(node) = self.layout.get(node_id) else {
            return;
        };

        if !node.visible || node.opacity <= 0.0 {
            return;
        }

        let bounds = node.absolute_bounds;
        let offset_x = bounds.x as f32;
        let offset_y = bounds.y as f32;

        // Get viewBox or use default
        let (vb_x, vb_y, vb_w, vb_h) = svg.view_box
            .as_ref()
            .map(|vb| (vb.min_x as f32, vb.min_y as f32, vb.width as f32, vb.height as f32))
            .unwrap_or((0.0, 0.0, 24.0, 24.0));

        // Calculate scale factors
        let scale_x = bounds.width as f32 / vb_w;
        let scale_y = bounds.height as f32 / vb_h;

        // Render each path
        for path in &svg.paths {
            let fill_color = path.fill;
            let stroke_color = path.stroke;
            let stroke_width = path.stroke_width.unwrap_or(1.0) as f32 * scale_x.min(scale_y);

            // Track current position for relative commands
            let mut cur_x = 0.0f32;
            let mut cur_y = 0.0f32;
            let mut start_x = 0.0f32;
            let mut start_y = 0.0f32;
            let mut path_started = false;

            for cmd in &path.commands {
                match cmd {
                    SvgPathCommand::MoveTo { x, y } => {
                        let px = offset_x + (*x as f32 - vb_x) * scale_x;
                        let py = offset_y + (*y as f32 - vb_y) * scale_y;
                        self.content.move_to(px, py);
                        cur_x = *x as f32;
                        cur_y = *y as f32;
                        start_x = cur_x;
                        start_y = cur_y;
                        path_started = true;
                    }
                    SvgPathCommand::MoveToRel { dx, dy } => {
                        cur_x += *dx as f32;
                        cur_y += *dy as f32;
                        let px = offset_x + (cur_x - vb_x) * scale_x;
                        let py = offset_y + (cur_y - vb_y) * scale_y;
                        self.content.move_to(px, py);
                        start_x = cur_x;
                        start_y = cur_y;
                        path_started = true;
                    }
                    SvgPathCommand::LineTo { x, y } => {
                        let px = offset_x + (*x as f32 - vb_x) * scale_x;
                        let py = offset_y + (*y as f32 - vb_y) * scale_y;
                        self.content.line_to(px, py);
                        cur_x = *x as f32;
                        cur_y = *y as f32;
                    }
                    SvgPathCommand::LineToRel { dx, dy } => {
                        cur_x += *dx as f32;
                        cur_y += *dy as f32;
                        let px = offset_x + (cur_x - vb_x) * scale_x;
                        let py = offset_y + (cur_y - vb_y) * scale_y;
                        self.content.line_to(px, py);
                    }
                    SvgPathCommand::HorizontalTo { x } => {
                        cur_x = *x as f32;
                        let px = offset_x + (cur_x - vb_x) * scale_x;
                        let py = offset_y + (cur_y - vb_y) * scale_y;
                        self.content.line_to(px, py);
                    }
                    SvgPathCommand::HorizontalToRel { dx } => {
                        cur_x += *dx as f32;
                        let px = offset_x + (cur_x - vb_x) * scale_x;
                        let py = offset_y + (cur_y - vb_y) * scale_y;
                        self.content.line_to(px, py);
                    }
                    SvgPathCommand::VerticalTo { y } => {
                        cur_y = *y as f32;
                        let px = offset_x + (cur_x - vb_x) * scale_x;
                        let py = offset_y + (cur_y - vb_y) * scale_y;
                        self.content.line_to(px, py);
                    }
                    SvgPathCommand::VerticalToRel { dy } => {
                        cur_y += *dy as f32;
                        let px = offset_x + (cur_x - vb_x) * scale_x;
                        let py = offset_y + (cur_y - vb_y) * scale_y;
                        self.content.line_to(px, py);
                    }
                    SvgPathCommand::CubicTo { x1, y1, x2, y2, x, y } => {
                        let px1 = offset_x + (*x1 as f32 - vb_x) * scale_x;
                        let py1 = offset_y + (*y1 as f32 - vb_y) * scale_y;
                        let px2 = offset_x + (*x2 as f32 - vb_x) * scale_x;
                        let py2 = offset_y + (*y2 as f32 - vb_y) * scale_y;
                        let px = offset_x + (*x as f32 - vb_x) * scale_x;
                        let py = offset_y + (*y as f32 - vb_y) * scale_y;
                        self.content.cubic_to(px1, py1, px2, py2, px, py);
                        cur_x = *x as f32;
                        cur_y = *y as f32;
                    }
                    SvgPathCommand::CubicToRel { dx1, dy1, dx2, dy2, dx, dy } => {
                        let px1 = offset_x + (cur_x + *dx1 as f32 - vb_x) * scale_x;
                        let py1 = offset_y + (cur_y + *dy1 as f32 - vb_y) * scale_y;
                        let px2 = offset_x + (cur_x + *dx2 as f32 - vb_x) * scale_x;
                        let py2 = offset_y + (cur_y + *dy2 as f32 - vb_y) * scale_y;
                        cur_x += *dx as f32;
                        cur_y += *dy as f32;
                        let px = offset_x + (cur_x - vb_x) * scale_x;
                        let py = offset_y + (cur_y - vb_y) * scale_y;
                        self.content.cubic_to(px1, py1, px2, py2, px, py);
                    }
                    SvgPathCommand::SmoothCubicTo { x2, y2, x, y } => {
                        // For smooth curves, use current point as first control
                        let px1 = offset_x + (cur_x - vb_x) * scale_x;
                        let py1 = offset_y + (cur_y - vb_y) * scale_y;
                        let px2 = offset_x + (*x2 as f32 - vb_x) * scale_x;
                        let py2 = offset_y + (*y2 as f32 - vb_y) * scale_y;
                        let px = offset_x + (*x as f32 - vb_x) * scale_x;
                        let py = offset_y + (*y as f32 - vb_y) * scale_y;
                        self.content.cubic_to(px1, py1, px2, py2, px, py);
                        cur_x = *x as f32;
                        cur_y = *y as f32;
                    }
                    SvgPathCommand::SmoothCubicToRel { dx2, dy2, dx, dy } => {
                        let px1 = offset_x + (cur_x - vb_x) * scale_x;
                        let py1 = offset_y + (cur_y - vb_y) * scale_y;
                        let px2 = offset_x + (cur_x + *dx2 as f32 - vb_x) * scale_x;
                        let py2 = offset_y + (cur_y + *dy2 as f32 - vb_y) * scale_y;
                        cur_x += *dx as f32;
                        cur_y += *dy as f32;
                        let px = offset_x + (cur_x - vb_x) * scale_x;
                        let py = offset_y + (cur_y - vb_y) * scale_y;
                        self.content.cubic_to(px1, py1, px2, py2, px, py);
                    }
                    SvgPathCommand::QuadTo { x1, y1, x, y } => {
                        // Convert quadratic to cubic bezier
                        let qx1 = *x1 as f32;
                        let qy1 = *y1 as f32;
                        let qx = *x as f32;
                        let qy = *y as f32;
                        // Cubic control points from quadratic
                        let cx1 = cur_x + 2.0 / 3.0 * (qx1 - cur_x);
                        let cy1 = cur_y + 2.0 / 3.0 * (qy1 - cur_y);
                        let cx2 = qx + 2.0 / 3.0 * (qx1 - qx);
                        let cy2 = qy + 2.0 / 3.0 * (qy1 - qy);

                        let px1 = offset_x + (cx1 - vb_x) * scale_x;
                        let py1 = offset_y + (cy1 - vb_y) * scale_y;
                        let px2 = offset_x + (cx2 - vb_x) * scale_x;
                        let py2 = offset_y + (cy2 - vb_y) * scale_y;
                        let px = offset_x + (qx - vb_x) * scale_x;
                        let py = offset_y + (qy - vb_y) * scale_y;
                        self.content.cubic_to(px1, py1, px2, py2, px, py);
                        cur_x = qx;
                        cur_y = qy;
                    }
                    SvgPathCommand::QuadToRel { dx1, dy1, dx, dy } => {
                        let qx1 = cur_x + *dx1 as f32;
                        let qy1 = cur_y + *dy1 as f32;
                        let qx = cur_x + *dx as f32;
                        let qy = cur_y + *dy as f32;
                        let cx1 = cur_x + 2.0 / 3.0 * (qx1 - cur_x);
                        let cy1 = cur_y + 2.0 / 3.0 * (qy1 - cur_y);
                        let cx2 = qx + 2.0 / 3.0 * (qx1 - qx);
                        let cy2 = qy + 2.0 / 3.0 * (qy1 - qy);

                        let px1 = offset_x + (cx1 - vb_x) * scale_x;
                        let py1 = offset_y + (cy1 - vb_y) * scale_y;
                        let px2 = offset_x + (cx2 - vb_x) * scale_x;
                        let py2 = offset_y + (cy2 - vb_y) * scale_y;
                        let px = offset_x + (qx - vb_x) * scale_x;
                        let py = offset_y + (qy - vb_y) * scale_y;
                        self.content.cubic_to(px1, py1, px2, py2, px, py);
                        cur_x = qx;
                        cur_y = qy;
                    }
                    SvgPathCommand::SmoothQuadTo { x, y } => {
                        // Simplified: use line for smooth quad without tracking previous control
                        let px = offset_x + (*x as f32 - vb_x) * scale_x;
                        let py = offset_y + (*y as f32 - vb_y) * scale_y;
                        self.content.line_to(px, py);
                        cur_x = *x as f32;
                        cur_y = *y as f32;
                    }
                    SvgPathCommand::SmoothQuadToRel { dx, dy } => {
                        cur_x += *dx as f32;
                        cur_y += *dy as f32;
                        let px = offset_x + (cur_x - vb_x) * scale_x;
                        let py = offset_y + (cur_y - vb_y) * scale_y;
                        self.content.line_to(px, py);
                    }
                    SvgPathCommand::ArcTo { rx, ry, x_rotation, large_arc, sweep, x, y } => {
                        // Approximate arc with cubic beziers
                        self.draw_arc(
                            cur_x, cur_y, *x as f32, *y as f32,
                            *rx as f32, *ry as f32, *x_rotation as f32,
                            *large_arc, *sweep,
                            offset_x, offset_y, vb_x, vb_y, scale_x, scale_y,
                        );
                        cur_x = *x as f32;
                        cur_y = *y as f32;
                    }
                    SvgPathCommand::ArcToRel { rx, ry, x_rotation, large_arc, sweep, dx, dy } => {
                        let end_x = cur_x + *dx as f32;
                        let end_y = cur_y + *dy as f32;
                        self.draw_arc(
                            cur_x, cur_y, end_x, end_y,
                            *rx as f32, *ry as f32, *x_rotation as f32,
                            *large_arc, *sweep,
                            offset_x, offset_y, vb_x, vb_y, scale_x, scale_y,
                        );
                        cur_x = end_x;
                        cur_y = end_y;
                    }
                    SvgPathCommand::ClosePath => {
                        self.content.close_path();
                        cur_x = start_x;
                        cur_y = start_y;
                    }
                }
            }

            // Apply fill and/or stroke
            if path_started {
                if let Some(color) = fill_color {
                    self.content.set_fill_rgb(color.r, color.g, color.b);
                }
                if let Some(color) = stroke_color {
                    self.content.set_stroke_rgb(color.r, color.g, color.b);
                    self.content.set_line_width(stroke_width);
                }

                match (fill_color, stroke_color) {
                    (Some(_), Some(_)) => { self.content.fill_nonzero_and_stroke(); }
                    (Some(_), None) => { self.content.fill_nonzero(); }
                    (None, Some(_)) => { self.content.stroke(); }
                    (None, None) => {}
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_arc(
        &mut self,
        x1: f32, y1: f32, x2: f32, y2: f32,
        rx: f32, ry: f32, _x_rotation: f32,
        _large_arc: bool, _sweep: bool,
        offset_x: f32, offset_y: f32,
        vb_x: f32, vb_y: f32,
        scale_x: f32, scale_y: f32,
    ) {
        // Simplified arc: just draw a line for now
        // Full arc implementation would use endpoint parameterization
        let px = offset_x + (x2 - vb_x) * scale_x;
        let py = offset_y + (y2 - vb_y) * scale_y;

        if rx > 0.0 && ry > 0.0 {
            // For small arcs, approximate with a quadratic bezier
            let mid_x = (x1 + x2) / 2.0;
            let mid_y = (y1 + y2) / 2.0;
            let ctrl_x = mid_x;
            let ctrl_y = mid_y - ry * 0.5; // Approximation

            // Convert to cubic
            let cx1 = x1 + 2.0 / 3.0 * (ctrl_x - x1);
            let cy1 = y1 + 2.0 / 3.0 * (ctrl_y - y1);
            let cx2 = x2 + 2.0 / 3.0 * (ctrl_x - x2);
            let cy2 = y2 + 2.0 / 3.0 * (ctrl_y - y2);

            let px1 = offset_x + (cx1 - vb_x) * scale_x;
            let py1 = offset_y + (cy1 - vb_y) * scale_y;
            let px2 = offset_x + (cx2 - vb_x) * scale_x;
            let py2 = offset_y + (cy2 - vb_y) * scale_y;

            self.content.cubic_to(px1, py1, px2, py2, px, py);
        } else {
            self.content.line_to(px, py);
        }
    }

    fn draw_rounded_rect(&mut self, x: f32, y: f32, w: f32, h: f32, radius: &CornerRadius) {
        // Bezier control point factor for approximating circles
        let k = 0.5522847498;

        let tl = radius.top_left.min(w / 2.0).min(h / 2.0);
        let tr = radius.top_right.min(w / 2.0).min(h / 2.0);
        let br = radius.bottom_right.min(w / 2.0).min(h / 2.0);
        let bl = radius.bottom_left.min(w / 2.0).min(h / 2.0);

        // Start at top-left corner (after the rounded part)
        self.content.move_to(x + tl, y);

        // Top edge
        self.content.line_to(x + w - tr, y);

        // Top-right corner
        if tr > 0.0 {
            self.content.cubic_to(
                x + w - tr + tr * k, y,
                x + w, y + tr - tr * k,
                x + w, y + tr,
            );
        }

        // Right edge
        self.content.line_to(x + w, y + h - br);

        // Bottom-right corner
        if br > 0.0 {
            self.content.cubic_to(
                x + w, y + h - br + br * k,
                x + w - br + br * k, y + h,
                x + w - br, y + h,
            );
        }

        // Bottom edge
        self.content.line_to(x + bl, y + h);

        // Bottom-left corner
        if bl > 0.0 {
            self.content.cubic_to(
                x + bl - bl * k, y + h,
                x, y + h - bl + bl * k,
                x, y + h - bl,
            );
        }

        // Left edge
        self.content.line_to(x, y + tl);

        // Top-left corner
        if tl > 0.0 {
            self.content.cubic_to(
                x, y + tl - tl * k,
                x + tl - tl * k, y,
                x + tl, y,
            );
        }

        self.content.close_path();
    }

    fn render_image(&mut self, _image: &seed_core::ast::ImageElement, node_id: LayoutNodeId) {
        let Some(node) = self.layout.get(node_id) else {
            return;
        };

        if !node.visible || node.opacity <= 0.0 {
            return;
        }

        let bounds = node.absolute_bounds;
        let x = bounds.x as f32;
        let y = bounds.y as f32;
        let w = bounds.width as f32;
        let h = bounds.height as f32;

        // For now, render a placeholder rectangle with X pattern
        // Full image embedding would require base64/stream handling
        self.content.set_fill_rgb(0.78, 0.78, 0.78);
        self.content.rect(x, y, w, h);
        self.content.fill_nonzero();

        // Draw X pattern
        self.content.set_stroke_rgb(0.6, 0.6, 0.6);
        self.content.set_line_width(1.0);
        self.content.move_to(x, y);
        self.content.line_to(x + w, y + h);
        self.content.stroke();
        self.content.move_to(x + w, y);
        self.content.line_to(x, y + h);
        self.content.stroke();
    }

    fn render_icon(&mut self, icon: &seed_core::ast::IconElement, node_id: LayoutNodeId) {
        let Some(node) = self.layout.get(node_id) else {
            return;
        };

        if !node.visible || node.opacity <= 0.0 {
            return;
        }

        let bounds = node.absolute_bounds;
        let x = bounds.x as f32;
        let y = bounds.y as f32;
        let w = bounds.width as f32;
        let h = bounds.height as f32;

        // Get color
        let color = icon.color.unwrap_or(Color::BLACK);

        match &icon.icon {
            seed_core::ast::IconSource::Svg(paths) => {
                // Calculate scale to fit icon in bounds
                let scale_x = w / 24.0;
                let scale_y = h / 24.0;

                for path in paths {
                    let fill_color = path.fill.unwrap_or(color);
                    self.content.set_fill_rgb(fill_color.r, fill_color.g, fill_color.b);

                    // Render SVG path commands (simplified)
                    for cmd in &path.commands {
                        use seed_core::ast::SvgPathCommand;
                        match cmd {
                            SvgPathCommand::MoveTo { x: px, y: py } => {
                                self.content.move_to(x + *px as f32 * scale_x, y + *py as f32 * scale_y);
                            }
                            SvgPathCommand::LineTo { x: px, y: py } => {
                                self.content.line_to(x + *px as f32 * scale_x, y + *py as f32 * scale_y);
                            }
                            SvgPathCommand::ClosePath => {
                                self.content.close_path();
                            }
                            // For other commands, skip (would need full path rendering)
                            _ => {}
                        }
                    }

                    self.content.fill_nonzero();
                }
            }
            _ => {
                // Named icons or token refs: render placeholder circle
                let cx = x + w / 2.0;
                let cy = y + h / 2.0;
                let r = w.min(h) / 2.0;

                self.content.set_fill_rgb(color.r, color.g, color.b);
                // Approximate circle with bezier curves
                let k = 0.5522847498;
                self.content.move_to(cx + r, cy);
                self.content.cubic_to(cx + r, cy + r * k, cx + r * k, cy + r, cx, cy + r);
                self.content.cubic_to(cx - r * k, cy + r, cx - r, cy + r * k, cx - r, cy);
                self.content.cubic_to(cx - r, cy - r * k, cx - r * k, cy - r, cx, cy - r);
                self.content.cubic_to(cx + r * k, cy - r, cx + r, cy - r * k, cx + r, cy);
                self.content.close_path();
                self.content.fill_nonzero();
            }
        }
    }
}

// Property extraction helpers

fn get_fill_color(properties: &[Property]) -> Option<Color> {
    get_color_property(properties, "fill")
        .or_else(|| get_color_property(properties, "background"))
        .or_else(|| get_color_property(properties, "background-color"))
}

fn get_stroke_color(properties: &[Property]) -> Option<Color> {
    get_color_property(properties, "stroke")
        .or_else(|| get_color_property(properties, "border-color"))
}

fn get_stroke_width(properties: &[Property]) -> f32 {
    get_length_property(properties, "stroke-width")
        .or_else(|| get_length_property(properties, "border-width"))
        .unwrap_or(1.0) as f32
}

fn get_corner_radius(properties: &[Property]) -> CornerRadius {
    if let Some(r) = get_length_property(properties, "corner-radius")
        .or_else(|| get_length_property(properties, "border-radius"))
    {
        return CornerRadius::uniform(r as f32);
    }

    let tl = get_length_property(properties, "corner-radius-top-left").unwrap_or(0.0) as f32;
    let tr = get_length_property(properties, "corner-radius-top-right").unwrap_or(0.0) as f32;
    let br = get_length_property(properties, "corner-radius-bottom-right").unwrap_or(0.0) as f32;
    let bl = get_length_property(properties, "corner-radius-bottom-left").unwrap_or(0.0) as f32;

    CornerRadius::new(tl, tr, br, bl)
}

fn is_rounded(radius: &CornerRadius) -> bool {
    radius.top_left > 0.0
        || radius.top_right > 0.0
        || radius.bottom_right > 0.0
        || radius.bottom_left > 0.0
}

fn get_color_property(properties: &[Property], name: &str) -> Option<Color> {
    properties.iter().find(|p| p.name == name).and_then(|p| {
        match &p.value {
            PropertyValue::Color(c) => Some(*c),
            PropertyValue::String(s) => Color::from_hex(s),
            _ => None,
        }
    })
}

fn get_length_property(properties: &[Property], name: &str) -> Option<f64> {
    properties.iter().find(|p| p.name == name).and_then(|p| {
        match &p.value {
            PropertyValue::Length(l) => l.to_px(None),
            PropertyValue::Number(n) => Some(*n),
            _ => None,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use seed_core::ast::Span;

    fn empty_doc() -> Document {
        Document {
            meta: None,
            tokens: None,
            elements: vec![],
            span: Span::default(),
        }
    }

    #[test]
    fn test_export_empty_doc() {
        let doc = empty_doc();
        let layout = LayoutTree::new();
        let result = export(&doc, &layout);
        assert!(result.is_ok());
        let pdf_bytes = result.unwrap();
        // Check it starts with PDF header
        assert!(pdf_bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn test_pdf_options_default() {
        let options = PdfOptions::default();
        assert_eq!(options.width, 595.0); // A4 width
        assert_eq!(options.height, 842.0); // A4 height
        assert!(options.compress);
    }
}
