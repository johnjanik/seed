//! Zero-copy memory sharing for WASM.
//!
//! This module provides efficient memory sharing between Rust and JavaScript
//! without copying data. It uses SharedArrayBuffer when available for
//! cross-thread sharing.

use wasm_bindgen::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;

/// Handle to a shared memory buffer.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BufferHandle(u32);

/// Memory region descriptor for zero-copy access.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug)]
pub struct MemoryRegion {
    /// Offset in bytes from the start of WASM memory.
    pub offset: u32,
    /// Length in bytes.
    pub length: u32,
}

#[wasm_bindgen]
impl MemoryRegion {
    /// Create a new memory region.
    #[wasm_bindgen(constructor)]
    pub fn new(offset: u32, length: u32) -> Self {
        Self { offset, length }
    }

    /// Get the end offset (exclusive).
    pub fn end(&self) -> u32 {
        self.offset + self.length
    }
}

/// Buffer allocation tracking.
struct BufferInfo {
    offset: u32,
    length: u32,
    #[allow(dead_code)]
    alignment: u32,
}

thread_local! {
    static BUFFER_ALLOCATOR: RefCell<BufferAllocator> = RefCell::new(BufferAllocator::new());
}

/// Manages shared buffer allocations.
struct BufferAllocator {
    /// Next handle ID.
    next_id: u32,
    /// Allocated buffers.
    buffers: HashMap<u32, BufferInfo>,
    /// Free list (offset, length).
    free_list: Vec<(u32, u32)>,
    /// Total allocated bytes.
    total_allocated: u32,
    /// High water mark.
    peak_allocated: u32,
    /// Next allocation offset (for non-wasm32 targets).
    next_offset: u32,
}

impl BufferAllocator {
    fn new() -> Self {
        Self {
            next_id: 1,
            buffers: HashMap::new(),
            free_list: Vec::new(),
            total_allocated: 0,
            peak_allocated: 0,
            next_offset: 65536, // Start after first page
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn get_memory_size() -> u32 {
        let pages = core::arch::wasm32::memory_size(0) as u32;
        pages * 65536
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn get_memory_size() -> u32 {
        // For non-wasm targets, simulate unlimited memory
        u32::MAX / 2
    }

    #[cfg(target_arch = "wasm32")]
    fn grow_memory(pages: usize) -> bool {
        core::arch::wasm32::memory_grow(0, pages) != usize::MAX
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn grow_memory(_pages: usize) -> bool {
        true // Always succeed on non-wasm
    }

    fn allocate(&mut self, length: u32, alignment: u32) -> Option<(u32, u32)> {
        // Try to find a free block that fits
        for i in 0..self.free_list.len() {
            let (offset, free_len) = self.free_list[i];

            // Align the offset
            let aligned_offset = (offset + alignment - 1) & !(alignment - 1);
            let padding = aligned_offset - offset;

            if free_len >= length + padding {
                // Found a suitable block
                self.free_list.remove(i);

                // Return remaining space to free list if substantial
                let remaining = free_len - length - padding;
                if remaining >= 64 {
                    self.free_list.push((aligned_offset + length, remaining));
                }

                let id = self.next_id;
                self.next_id += 1;

                self.buffers.insert(id, BufferInfo {
                    offset: aligned_offset,
                    length,
                    alignment,
                });

                self.total_allocated += length;
                self.peak_allocated = self.peak_allocated.max(self.total_allocated);

                return Some((id, aligned_offset));
            }
        }

        // No free block found, allocate from end of WASM memory
        let id = self.next_id;
        self.next_id += 1;

        // Get current memory size
        let current_size = Self::get_memory_size();

        // Find the end of allocated memory
        let mut end = self.next_offset;
        for info in self.buffers.values() {
            end = end.max(info.offset + info.length);
        }

        // Align the new offset
        let aligned_offset = (end + alignment - 1) & !(alignment - 1);

        // Check if we need to grow memory
        let required_size = aligned_offset + length;
        if required_size > current_size {
            let needed_pages = ((required_size - current_size) + 65535) / 65536;
            if !Self::grow_memory(needed_pages as usize) {
                return None; // Memory grow failed
            }
        }

        self.buffers.insert(id, BufferInfo {
            offset: aligned_offset,
            length,
            alignment,
        });

        self.next_offset = aligned_offset + length;
        self.total_allocated += length;
        self.peak_allocated = self.peak_allocated.max(self.total_allocated);

        Some((id, aligned_offset))
    }

    fn deallocate(&mut self, id: u32) -> bool {
        if let Some(info) = self.buffers.remove(&id) {
            self.total_allocated -= info.length;
            self.free_list.push((info.offset, info.length));

            // Merge adjacent free blocks
            self.free_list.sort_by_key(|(offset, _)| *offset);
            let mut merged = Vec::new();
            for (offset, length) in self.free_list.drain(..) {
                if let Some((last_offset, last_length)) = merged.last_mut() {
                    if *last_offset + *last_length == offset {
                        *last_length += length;
                        continue;
                    }
                }
                merged.push((offset, length));
            }
            self.free_list = merged;

            true
        } else {
            false
        }
    }

    fn get_region(&self, id: u32) -> Option<MemoryRegion> {
        self.buffers.get(&id).map(|info| MemoryRegion {
            offset: info.offset,
            length: info.length,
        })
    }
}

/// Shared memory buffer manager.
///
/// Provides zero-copy memory sharing between Rust and JavaScript.
#[wasm_bindgen]
pub struct SharedMemory {
    _private: (),
}

#[wasm_bindgen]
impl SharedMemory {
    /// Create a new shared memory manager.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Check if SharedArrayBuffer is available.
    #[wasm_bindgen(js_name = isSharedArrayBufferSupported)]
    pub fn is_shared_array_buffer_supported() -> bool {
        // Check for SharedArrayBuffer support via JavaScript
        js_sys::eval("typeof SharedArrayBuffer !== 'undefined'")
            .map(|v| v.as_bool().unwrap_or(false))
            .unwrap_or(false)
    }

    /// Get the base address of WASM linear memory.
    #[wasm_bindgen(js_name = getMemoryBase)]
    pub fn get_memory_base() -> u32 {
        // The WASM linear memory starts at address 0
        0
    }

    /// Get the current size of WASM memory in bytes.
    #[wasm_bindgen(js_name = getMemorySize)]
    pub fn get_memory_size() -> u32 {
        BufferAllocator::get_memory_size()
    }

    /// Allocate a shared buffer with specified alignment.
    ///
    /// Returns a handle that can be used to get the memory region.
    #[wasm_bindgen]
    pub fn allocate(&self, length: u32, alignment: u32) -> Option<BufferHandle> {
        let alignment = alignment.max(1).next_power_of_two();

        BUFFER_ALLOCATOR.with(|alloc| {
            alloc.borrow_mut().allocate(length, alignment).map(|(id, _)| BufferHandle(id))
        })
    }

    /// Allocate a buffer suitable for image data (RGBA, 4-byte aligned).
    #[wasm_bindgen(js_name = allocateImage)]
    pub fn allocate_image(&self, width: u32, height: u32) -> Option<BufferHandle> {
        let length = width * height * 4; // RGBA
        self.allocate(length, 4)
    }

    /// Allocate a buffer suitable for vertices (float32, 4-byte aligned).
    #[wasm_bindgen(js_name = allocateVertices)]
    pub fn allocate_vertices(&self, count: u32, components_per_vertex: u32) -> Option<BufferHandle> {
        let length = count * components_per_vertex * 4; // float32
        self.allocate(length, 4)
    }

    /// Deallocate a shared buffer.
    #[wasm_bindgen]
    pub fn deallocate(&self, handle: BufferHandle) -> bool {
        BUFFER_ALLOCATOR.with(|alloc| alloc.borrow_mut().deallocate(handle.0))
    }

    /// Get the memory region for a buffer handle.
    #[wasm_bindgen(js_name = getRegion)]
    pub fn get_region(&self, handle: BufferHandle) -> Option<MemoryRegion> {
        BUFFER_ALLOCATOR.with(|alloc| alloc.borrow().get_region(handle.0))
    }

    /// Get allocation statistics.
    #[wasm_bindgen(js_name = getStats)]
    pub fn get_stats(&self) -> MemoryStats {
        BUFFER_ALLOCATOR.with(|alloc| {
            let alloc = alloc.borrow();
            MemoryStats {
                total_allocated: alloc.total_allocated,
                peak_allocated: alloc.peak_allocated,
                buffer_count: alloc.buffers.len() as u32,
                free_blocks: alloc.free_list.len() as u32,
            }
        })
    }

    /// Write bytes directly to a buffer.
    #[wasm_bindgen(js_name = writeBytes)]
    pub fn write_bytes(&self, handle: BufferHandle, data: &[u8]) -> bool {
        BUFFER_ALLOCATOR.with(|alloc| {
            if let Some(region) = alloc.borrow().get_region(handle.0) {
                if data.len() as u32 <= region.length {
                    let ptr = region.offset as *mut u8;
                    unsafe {
                        std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data.len());
                    }
                    return true;
                }
            }
            false
        })
    }

    /// Read bytes from a buffer.
    #[wasm_bindgen(js_name = readBytes)]
    pub fn read_bytes(&self, handle: BufferHandle) -> Option<Vec<u8>> {
        BUFFER_ALLOCATOR.with(|alloc| {
            if let Some(region) = alloc.borrow().get_region(handle.0) {
                let ptr = region.offset as *const u8;
                let slice = unsafe {
                    std::slice::from_raw_parts(ptr, region.length as usize)
                };
                return Some(slice.to_vec());
            }
            None
        })
    }

    /// Create a typed array view of a buffer (zero-copy).
    ///
    /// The returned object contains offset and length for creating
    /// a TypedArray view in JavaScript.
    #[wasm_bindgen(js_name = createView)]
    pub fn create_view(&self, handle: BufferHandle) -> Option<MemoryRegion> {
        self.get_region(handle)
    }
}

impl Default for SharedMemory {
    fn default() -> Self {
        Self::new()
    }
}

/// Memory allocation statistics.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug)]
pub struct MemoryStats {
    /// Total bytes currently allocated.
    pub total_allocated: u32,
    /// Peak bytes allocated.
    pub peak_allocated: u32,
    /// Number of active buffers.
    pub buffer_count: u32,
    /// Number of free blocks.
    pub free_blocks: u32,
}

/// Ring buffer for streaming data.
///
/// Useful for animation frames, audio samples, or other streaming use cases.
#[wasm_bindgen]
pub struct RingBuffer {
    handle: BufferHandle,
    capacity: u32,
    read_pos: u32,
    write_pos: u32,
}

#[wasm_bindgen]
impl RingBuffer {
    /// Create a new ring buffer with the specified capacity.
    #[wasm_bindgen(constructor)]
    pub fn new(capacity: u32) -> Result<RingBuffer, JsError> {
        let mem = SharedMemory::new();
        let handle = mem.allocate(capacity, 64)
            .ok_or_else(|| JsError::new("Failed to allocate ring buffer"))?;

        Ok(RingBuffer {
            handle,
            capacity,
            read_pos: 0,
            write_pos: 0,
        })
    }

    /// Get the capacity of the ring buffer.
    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Get the number of bytes available to read.
    #[wasm_bindgen(js_name = available)]
    pub fn available(&self) -> u32 {
        if self.write_pos >= self.read_pos {
            self.write_pos - self.read_pos
        } else {
            self.capacity - self.read_pos + self.write_pos
        }
    }

    /// Get the number of bytes that can be written.
    #[wasm_bindgen(js_name = freeSpace)]
    pub fn free_space(&self) -> u32 {
        self.capacity - self.available() - 1
    }

    /// Write data to the ring buffer.
    pub fn write(&mut self, data: &[u8]) -> u32 {
        let to_write = (data.len() as u32).min(self.free_space());
        if to_write == 0 {
            return 0;
        }

        BUFFER_ALLOCATOR.with(|alloc| {
            if let Some(region) = alloc.borrow().get_region(self.handle.0) {
                let base = region.offset as *mut u8;

                let mut written = 0u32;
                for &byte in data.iter().take(to_write as usize) {
                    let pos = (self.write_pos + written) % self.capacity;
                    unsafe {
                        *base.add(pos as usize) = byte;
                    }
                    written += 1;
                }

                self.write_pos = (self.write_pos + written) % self.capacity;
                written
            } else {
                0
            }
        })
    }

    /// Read data from the ring buffer.
    pub fn read(&mut self, length: u32) -> Vec<u8> {
        let to_read = length.min(self.available());
        if to_read == 0 {
            return Vec::new();
        }

        BUFFER_ALLOCATOR.with(|alloc| {
            if let Some(region) = alloc.borrow().get_region(self.handle.0) {
                let base = region.offset as *const u8;

                let mut result = Vec::with_capacity(to_read as usize);
                for i in 0..to_read {
                    let pos = (self.read_pos + i) % self.capacity;
                    unsafe {
                        result.push(*base.add(pos as usize));
                    }
                }

                self.read_pos = (self.read_pos + to_read) % self.capacity;
                result
            } else {
                Vec::new()
            }
        })
    }

    /// Reset the ring buffer.
    pub fn reset(&mut self) {
        self.read_pos = 0;
        self.write_pos = 0;
    }

    /// Get the underlying buffer handle.
    #[wasm_bindgen(getter)]
    pub fn handle(&self) -> BufferHandle {
        self.handle
    }

    /// Get the memory region for zero-copy access.
    #[wasm_bindgen(js_name = getRegion)]
    pub fn get_region(&self) -> Option<MemoryRegion> {
        BUFFER_ALLOCATOR.with(|alloc| alloc.borrow().get_region(self.handle.0))
    }
}

impl Drop for RingBuffer {
    fn drop(&mut self) {
        let mem = SharedMemory::new();
        mem.deallocate(self.handle);
    }
}

/// Double buffer for frame data.
///
/// Provides lock-free double buffering for render data.
#[wasm_bindgen]
pub struct DoubleBuffer {
    front: BufferHandle,
    back: BufferHandle,
    buffer_size: u32,
    swapped: bool,
}

#[wasm_bindgen]
impl DoubleBuffer {
    /// Create a new double buffer.
    #[wasm_bindgen(constructor)]
    pub fn new(size: u32) -> Result<DoubleBuffer, JsError> {
        let mem = SharedMemory::new();
        let front = mem.allocate(size, 64)
            .ok_or_else(|| JsError::new("Failed to allocate front buffer"))?;
        let back = mem.allocate(size, 64)
            .ok_or_else(|| JsError::new("Failed to allocate back buffer"))?;

        Ok(DoubleBuffer {
            front,
            back,
            buffer_size: size,
            swapped: false,
        })
    }

    /// Get the front buffer (for reading/display).
    #[wasm_bindgen(js_name = getFrontRegion)]
    pub fn get_front_region(&self) -> Option<MemoryRegion> {
        let handle = if self.swapped { self.back } else { self.front };
        SharedMemory::new().get_region(handle)
    }

    /// Get the back buffer (for writing).
    #[wasm_bindgen(js_name = getBackRegion)]
    pub fn get_back_region(&self) -> Option<MemoryRegion> {
        let handle = if self.swapped { self.front } else { self.back };
        SharedMemory::new().get_region(handle)
    }

    /// Swap front and back buffers.
    pub fn swap(&mut self) {
        self.swapped = !self.swapped;
    }

    /// Get buffer size.
    #[wasm_bindgen(getter)]
    pub fn size(&self) -> u32 {
        self.buffer_size
    }
}

impl Drop for DoubleBuffer {
    fn drop(&mut self) {
        let mem = SharedMemory::new();
        mem.deallocate(self.front);
        mem.deallocate(self.back);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_region() {
        let region = MemoryRegion::new(100, 50);
        assert_eq!(region.offset, 100);
        assert_eq!(region.length, 50);
        assert_eq!(region.end(), 150);
    }
}
