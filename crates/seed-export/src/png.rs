//! PNG export for 2D documents.

use seed_core::{Document, ExportError};
use seed_layout::LayoutTree;
use seed_render_2d::SoftwareRenderer;

/// Export options for PNG.
#[derive(Debug, Clone)]
pub struct PngOptions {
    /// Scale factor (1.0 = 1:1, 2.0 = 2x resolution).
    pub scale: f32,
    /// Background color as RGBA.
    pub background: [u8; 4],
}

impl Default for PngOptions {
    fn default() -> Self {
        Self {
            scale: 1.0,
            background: [255, 255, 255, 255], // White
        }
    }
}

/// Export a document to PNG.
pub fn export(doc: &Document, layout: &LayoutTree) -> Result<Vec<u8>, ExportError> {
    export_with_options(doc, layout, &PngOptions::default())
}

/// Export a document to PNG with custom options.
pub fn export_with_options(
    doc: &Document,
    layout: &LayoutTree,
    options: &PngOptions,
) -> Result<Vec<u8>, ExportError> {
    let bounds = layout.content_bounds();
    let width = ((bounds.width.max(1.0)) * options.scale as f64) as u32;
    let height = ((bounds.height.max(1.0)) * options.scale as f64) as u32;

    // Render using software renderer
    let mut renderer = SoftwareRenderer::new(width, height);
    renderer.clear(
        options.background[0],
        options.background[1],
        options.background[2],
        options.background[3],
    );

    let pixels = renderer.render(doc, layout)
        .map_err(|e| ExportError::RenderFailed {
            reason: format!("{}", e),
        })?;

    // Encode as PNG
    encode_png(pixels, width, height)
}

/// Encode raw RGBA pixels as PNG.
fn encode_png(pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>, ExportError> {
    let mut output = Vec::new();

    // PNG signature
    output.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

    // IHDR chunk
    let mut ihdr_data = Vec::new();
    ihdr_data.extend_from_slice(&width.to_be_bytes());
    ihdr_data.extend_from_slice(&height.to_be_bytes());
    ihdr_data.push(8);  // Bit depth
    ihdr_data.push(6);  // Color type: RGBA
    ihdr_data.push(0);  // Compression method
    ihdr_data.push(0);  // Filter method
    ihdr_data.push(0);  // Interlace method
    write_chunk(&mut output, b"IHDR", &ihdr_data);

    // Prepare image data with filter bytes
    let row_size = width as usize * 4;
    let mut raw_data = Vec::with_capacity((height as usize) * (row_size + 1));
    for y in 0..height as usize {
        raw_data.push(0); // No filter
        raw_data.extend_from_slice(&pixels[y * row_size..(y + 1) * row_size]);
    }

    // Compress with deflate
    let compressed = compress_deflate(&raw_data);

    // IDAT chunk
    write_chunk(&mut output, b"IDAT", &compressed);

    // IEND chunk
    write_chunk(&mut output, b"IEND", &[]);

    Ok(output)
}

/// Write a PNG chunk.
fn write_chunk(output: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
    // Length
    output.extend_from_slice(&(data.len() as u32).to_be_bytes());
    // Type
    output.extend_from_slice(chunk_type);
    // Data
    output.extend_from_slice(data);
    // CRC
    let crc = crc32(&[chunk_type.as_slice(), data].concat());
    output.extend_from_slice(&crc.to_be_bytes());
}

/// Calculate CRC32 for PNG.
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for byte in data {
        let index = ((crc ^ (*byte as u32)) & 0xFF) as usize;
        crc = CRC_TABLE[index] ^ (crc >> 8);
    }
    crc ^ 0xFFFFFFFF
}

/// Simple deflate compression (stores data uncompressed).
/// For proper compression, use a deflate library.
fn compress_deflate(data: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();

    // Zlib header
    output.push(0x78); // CMF: deflate, 32K window
    output.push(0x01); // FLG: no dict, fastest

    // Split into blocks (max 65535 bytes each)
    let mut remaining = data;
    while !remaining.is_empty() {
        let block_size = remaining.len().min(65535);
        let is_final = block_size == remaining.len();

        // Block header
        output.push(if is_final { 0x01 } else { 0x00 }); // BFINAL + BTYPE (stored)
        output.extend_from_slice(&(block_size as u16).to_le_bytes());
        output.extend_from_slice(&(!(block_size as u16)).to_le_bytes());

        // Block data
        output.extend_from_slice(&remaining[..block_size]);
        remaining = &remaining[block_size..];
    }

    // Adler-32 checksum
    let checksum = adler32(data);
    output.extend_from_slice(&checksum.to_be_bytes());

    output
}

/// Calculate Adler-32 checksum.
fn adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;

    for byte in data {
        a = (a + *byte as u32) % 65521;
        b = (b + a) % 65521;
    }

    (b << 16) | a
}

/// CRC32 lookup table for PNG.
static CRC_TABLE: [u32; 256] = [
    0x00000000, 0x77073096, 0xee0e612c, 0x990951ba, 0x076dc419, 0x706af48f,
    0xe963a535, 0x9e6495a3, 0x0edb8832, 0x79dcb8a4, 0xe0d5e91e, 0x97d2d988,
    0x09b64c2b, 0x7eb17cbd, 0xe7b82d07, 0x90bf1d91, 0x1db71064, 0x6ab020f2,
    0xf3b97148, 0x84be41de, 0x1adad47d, 0x6ddde4eb, 0xf4d4b551, 0x83d385c7,
    0x136c9856, 0x646ba8c0, 0xfd62f97a, 0x8a65c9ec, 0x14015c4f, 0x63066cd9,
    0xfa0f3d63, 0x8d080df5, 0x3b6e20c8, 0x4c69105e, 0xd56041e4, 0xa2677172,
    0x3c03e4d1, 0x4b04d447, 0xd20d85fd, 0xa50ab56b, 0x35b5a8fa, 0x42b2986c,
    0xdbbbc9d6, 0xacbcf940, 0x32d86ce3, 0x45df5c75, 0xdcd60dcf, 0xabd13d59,
    0x26d930ac, 0x51de003a, 0xc8d75180, 0xbfd06116, 0x21b4f4b5, 0x56b3c423,
    0xcfba9599, 0xb8bda50f, 0x2802b89e, 0x5f058808, 0xc60cd9b2, 0xb10be924,
    0x2f6f7c87, 0x58684c11, 0xc1611dab, 0xb6662d3d, 0x76dc4190, 0x01db7106,
    0x98d220bc, 0xefd5102a, 0x71b18589, 0x06b6b51f, 0x9fbfe4a5, 0xe8b8d433,
    0x7807c9a2, 0x0f00f934, 0x9609a88e, 0xe10e9818, 0x7f6a0dbb, 0x086d3d2d,
    0x91646c97, 0xe6635c01, 0x6b6b51f4, 0x1c6c6162, 0x856530d8, 0xf262004e,
    0x6c0695ed, 0x1b01a57b, 0x8208f4c1, 0xf50fc457, 0x65b0d9c6, 0x12b7e950,
    0x8bbeb8ea, 0xfcb9887c, 0x62dd1ddf, 0x15da2d49, 0x8cd37cf3, 0xfbd44c65,
    0x4db26158, 0x3ab551ce, 0xa3bc0074, 0xd4bb30e2, 0x4adfa541, 0x3dd895d7,
    0xa4d1c46d, 0xd3d6f4fb, 0x4369e96a, 0x346ed9fc, 0xad678846, 0xda60b8d0,
    0x44042d73, 0x33031de5, 0xaa0a4c5f, 0xdd0d7a0b, 0x5005713c, 0x270241aa,
    0xbe0b1010, 0xc90c2086, 0x5768b525, 0x206f85b3, 0xb966d409, 0xce61e49f,
    0x5edef90e, 0x29d9c998, 0xb0d09822, 0xc7d7a8b4, 0x59b33d17, 0x2eb40d81,
    0xb7bd5c3b, 0xc0ba6cad, 0xedb88320, 0x9abfb3b6, 0x03b6e20c, 0x74b1d29a,
    0xead54739, 0x9dd277af, 0x04db2615, 0x73dc1683, 0xe3630b12, 0x94643b84,
    0x0d6d6a3e, 0x7a6a5aa8, 0xe40ecf0b, 0x9309ff9d, 0x0a00ae27, 0x7d079eb1,
    0xf00f9344, 0x8708a3d2, 0x1e01f268, 0x6906c2fe, 0xf762575d, 0x806567cb,
    0x196c3671, 0x6e6b06e7, 0xfed41b76, 0x89d32be0, 0x10da7a5a, 0x67dd4acc,
    0xf9b9df6f, 0x8ebeeff9, 0x17b7be43, 0x60b08ed5, 0xd6d6a3e8, 0xa1d1937e,
    0x38d8c2c4, 0x4fdff252, 0xd1bb67f1, 0xa6bc5767, 0x3fb506dd, 0x48b2364b,
    0xd80d2bda, 0xaf0a1b4c, 0x36034af6, 0x41047a60, 0xdf60efc3, 0xa867df55,
    0x316e8eef, 0x4669be79, 0xcb61b38c, 0xbc66831a, 0x256fd2a0, 0x5268e236,
    0xcc0c7795, 0xbb0b4703, 0x220216b9, 0x5505262f, 0xc5ba3bbe, 0xb2bd0b28,
    0x2bb45a92, 0x5cb36a04, 0xc2d7ffa7, 0xb5d0cf31, 0x2cd99e8b, 0x5bdeae1d,
    0x9b64c2b0, 0xec63f226, 0x756aa39c, 0x026d930a, 0x9c0906a9, 0xeb0e363f,
    0x72076785, 0x05005713, 0x95bf4a82, 0xe2b87a14, 0x7bb12bae, 0x0cb61b38,
    0x92d28e9b, 0xe5d5be0d, 0x7cdcefb7, 0x0bdbdf21, 0x86d3d2d4, 0xf1d4e242,
    0x68ddb3f8, 0x1fda836e, 0x81be16cd, 0xf6b9265b, 0x6fb077e1, 0x18b74777,
    0x88085ae6, 0xff0f6a70, 0x66063bca, 0x11010b5c, 0x8f659eff, 0xf862ae69,
    0x616bffd3, 0x166ccf45, 0xa00ae278, 0xd70dd2ee, 0x4e048354, 0x3903b3c2,
    0xa7672661, 0xd06016f7, 0x4969474d, 0x3e6e77db, 0xaed16a4a, 0xd9d65adc,
    0x40df0b66, 0x37d83bf0, 0xa9bcae53, 0xdebb9ec5, 0x47b2cf7f, 0x30b5ffe9,
    0xbdbdf21c, 0xcabac28a, 0x53b39330, 0x24b4a3a6, 0xbad03605, 0xcdd706b3,
    0x54de5729, 0x23d967bf, 0xb3667a2e, 0xc4614ab8, 0x5d681b02, 0x2a6f2b94,
    0xb40bbe37, 0xc30c8ea1, 0x5a05df1b, 0x2d02ef8d,
];

#[cfg(test)]
mod tests {
    use super::*;
    use seed_core::ast::Span;

    #[test]
    fn test_crc32() {
        // Test with known value
        let data = b"123456789";
        assert_eq!(crc32(data), 0xCBF43926);
    }

    #[test]
    fn test_adler32() {
        let data = b"Wikipedia";
        assert_eq!(adler32(data), 0x11E60398);
    }

    #[test]
    fn test_png_signature() {
        let doc = Document {
            meta: None,
            tokens: None,
            elements: vec![],
            span: Span::default(),
        };
        let layout = LayoutTree::new();

        let png = export(&doc, &layout).unwrap();

        // Check PNG signature
        assert_eq!(&png[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }

    #[test]
    fn test_png_has_ihdr() {
        let doc = Document {
            meta: None,
            tokens: None,
            elements: vec![],
            span: Span::default(),
        };
        let layout = LayoutTree::new();

        let png = export(&doc, &layout).unwrap();

        // Find IHDR chunk (should be right after signature)
        assert_eq!(&png[12..16], b"IHDR");
    }
}
