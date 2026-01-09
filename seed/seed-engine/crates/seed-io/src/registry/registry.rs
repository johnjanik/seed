//! Format registry for managing readers and writers.

use crate::error::{IoError, Result};
use crate::scene::UnifiedScene;
use indexmap::IndexMap;

use super::traits::{FormatReader, FormatWriter, ReadOptions, WriteOptions};

/// Registry of format readers and writers.
///
/// The registry manages format handlers and provides auto-detection
/// for reading files and format selection for writing.
pub struct FormatRegistry {
    readers: IndexMap<String, Box<dyn FormatReader>>,
    writers: IndexMap<String, Box<dyn FormatWriter>>,
    extension_to_reader: IndexMap<String, String>,
    extension_to_writer: IndexMap<String, String>,
}

impl Default for FormatRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            readers: IndexMap::new(),
            writers: IndexMap::new(),
            extension_to_reader: IndexMap::new(),
            extension_to_writer: IndexMap::new(),
        }
    }

    /// Create a registry with default format handlers.
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();

        // Register Seed format
        #[cfg(feature = "seed")]
        {
            registry.register_reader(crate::formats::seed::SeedReader::new());
            registry.register_writer(crate::formats::seed::SeedWriter::new());
        }

        // Register glTF format
        #[cfg(feature = "gltf")]
        {
            registry.register_reader(crate::formats::gltf::GltfReader::new());
            registry.register_writer(crate::formats::gltf::GltfWriter::new());
        }

        // Register STEP format
        #[cfg(feature = "step")]
        {
            registry.register_reader(crate::formats::step::StepReader::new());
            registry.register_writer(crate::formats::step::StepWriter::new());
        }

        // Register USD format
        #[cfg(feature = "usd")]
        {
            registry.register_reader(crate::formats::usd::UsdReader::new());
            registry.register_writer(crate::formats::usd::UsdWriter::new());
        }

        registry
    }

    /// Register a format reader.
    pub fn register_reader<R: FormatReader + 'static>(&mut self, reader: R) {
        let name = reader.name().to_lowercase();

        // Map extensions to this reader
        for ext in reader.extensions() {
            self.extension_to_reader
                .insert(ext.to_lowercase(), name.clone());
        }

        self.readers.insert(name, Box::new(reader));
    }

    /// Register a format writer.
    pub fn register_writer<W: FormatWriter + 'static>(&mut self, writer: W) {
        let name = writer.name().to_lowercase();

        // Map extension to this writer
        self.extension_to_writer
            .insert(writer.extension().to_lowercase(), name.clone());

        self.writers.insert(name, Box::new(writer));
    }

    /// Get a reader by format name.
    pub fn get_reader(&self, format: &str) -> Option<&dyn FormatReader> {
        self.readers.get(&format.to_lowercase()).map(|r| r.as_ref())
    }

    /// Get a writer by format name.
    pub fn get_writer(&self, format: &str) -> Option<&dyn FormatWriter> {
        self.writers.get(&format.to_lowercase()).map(|w| w.as_ref())
    }

    /// Get a reader by file extension.
    pub fn reader_for_extension(&self, ext: &str) -> Option<&dyn FormatReader> {
        let ext_lower = ext.trim_start_matches('.').to_lowercase();
        let format = self.extension_to_reader.get(&ext_lower)?;
        self.get_reader(format)
    }

    /// Get a writer by file extension.
    pub fn writer_for_extension(&self, ext: &str) -> Option<&dyn FormatWriter> {
        let ext_lower = ext.trim_start_matches('.').to_lowercase();
        let format = self.extension_to_writer.get(&ext_lower)?;
        self.get_writer(format)
    }

    /// List all registered reader format names.
    pub fn reader_formats(&self) -> impl Iterator<Item = &str> {
        self.readers.keys().map(String::as_str)
    }

    /// List all registered writer format names.
    pub fn writer_formats(&self) -> impl Iterator<Item = &str> {
        self.writers.keys().map(String::as_str)
    }

    /// Read data with auto-detection.
    ///
    /// Tries each registered reader's `can_read` method to find a compatible format.
    pub fn read(&self, data: &[u8], options: &ReadOptions) -> Result<UnifiedScene> {
        // Try each reader's can_read
        for reader in self.readers.values() {
            if reader.can_read(data) {
                return reader.read(data, options);
            }
        }

        Err(IoError::UnknownFormat(
            "no reader recognized this format".into(),
        ))
    }

    /// Read data with explicit format hint.
    pub fn read_as(&self, data: &[u8], format: &str, options: &ReadOptions) -> Result<UnifiedScene> {
        let reader = self
            .get_reader(format)
            .ok_or_else(|| IoError::NoReader(format.into()))?;
        reader.read(data, options)
    }

    /// Read data with file extension hint.
    pub fn read_with_extension(
        &self,
        data: &[u8],
        extension: &str,
        options: &ReadOptions,
    ) -> Result<UnifiedScene> {
        let reader = self
            .reader_for_extension(extension)
            .ok_or_else(|| IoError::NoReader(extension.into()))?;
        reader.read(data, options)
    }

    /// Write scene to a format.
    pub fn write(
        &self,
        scene: &UnifiedScene,
        format: &str,
        options: &WriteOptions,
    ) -> Result<Vec<u8>> {
        let writer = self
            .get_writer(format)
            .ok_or_else(|| IoError::NoWriter(format.into()))?;
        writer.write(scene, options)
    }

    /// Write scene with file extension hint.
    pub fn write_with_extension(
        &self,
        scene: &UnifiedScene,
        extension: &str,
        options: &WriteOptions,
    ) -> Result<Vec<u8>> {
        let writer = self
            .writer_for_extension(extension)
            .ok_or_else(|| IoError::NoWriter(extension.into()))?;
        writer.write(scene, options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockReader;

    impl FormatReader for MockReader {
        fn name(&self) -> &'static str {
            "mock"
        }

        fn extensions(&self) -> &[&'static str] {
            &["mock", "mck"]
        }

        fn can_read(&self, data: &[u8]) -> bool {
            data.starts_with(b"MOCK")
        }

        fn read(&self, _data: &[u8], _options: &ReadOptions) -> Result<UnifiedScene> {
            Ok(UnifiedScene::new())
        }
    }

    struct MockWriter;

    impl FormatWriter for MockWriter {
        fn name(&self) -> &'static str {
            "mock"
        }

        fn extension(&self) -> &'static str {
            "mock"
        }

        fn write(&self, _scene: &UnifiedScene, _options: &WriteOptions) -> Result<Vec<u8>> {
            Ok(b"MOCK".to_vec())
        }
    }

    #[test]
    fn test_register_reader() {
        let mut registry = FormatRegistry::new();
        registry.register_reader(MockReader);

        assert!(registry.get_reader("mock").is_some());
        assert!(registry.get_reader("Mock").is_some()); // case insensitive
        assert!(registry.reader_for_extension("mock").is_some());
        assert!(registry.reader_for_extension(".mck").is_some());
    }

    #[test]
    fn test_register_writer() {
        let mut registry = FormatRegistry::new();
        registry.register_writer(MockWriter);

        assert!(registry.get_writer("mock").is_some());
        assert!(registry.writer_for_extension("mock").is_some());
    }

    #[test]
    fn test_auto_detect() {
        let mut registry = FormatRegistry::new();
        registry.register_reader(MockReader);

        let data = b"MOCK content";
        let result = registry.read(data, &ReadOptions::default());
        assert!(result.is_ok());

        let unknown = b"UNKNOWN";
        let result = registry.read(unknown, &ReadOptions::default());
        assert!(matches!(result, Err(IoError::UnknownFormat(_))));
    }
}
