use super::arc_block::ArcBlock;
use core::{alloc::Layout, slice};

#[derive(Debug)]
#[repr(C)]
pub struct ArcBuffer {
    block: ArcBlock,
}

#[repr(C)]
struct ArcBufferInner {
    length: usize,
    first_byte: u8,
}

impl ArcBuffer {
    pub fn new(length: usize) -> Self {
        Self {
            block: if length == 0 {
                ArcBlock::new(Layout::from_size_align(0, 1).unwrap())
            } else {
                let mut block = ArcBlock::new(
                    Layout::new::<usize>()
                        .extend(Layout::array::<u8>(length).unwrap())
                        .unwrap()
                        .0
                        .pad_to_align(),
                );

                unsafe { *(block.ptr_mut() as *mut usize) = length }

                block
            },
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe {
            if self.block.is_null() {
                slice::from_raw_parts(core::ptr::NonNull::dangling().as_ptr(), 0)
            } else {
                let inner = &*(self.block.ptr() as *const ArcBufferInner);

                slice::from_raw_parts(&inner.first_byte, inner.length)
            }
        }
    }

    pub fn as_slice_mut(&mut self) -> &mut [u8] {
        unsafe {
            if self.block.is_null() {
                slice::from_raw_parts_mut(core::ptr::NonNull::dangling().as_ptr(), 0)
            } else {
                let inner = &mut *(self.block.ptr() as *mut ArcBufferInner);

                slice::from_raw_parts_mut(&mut inner.first_byte, inner.length)
            }
        }
    }
}

impl Default for ArcBuffer {
    fn default() -> Self {
        Self::new(0)
    }
}

impl Clone for ArcBuffer {
    fn clone(&self) -> Self {
        Self {
            block: self.block.clone(),
        }
    }
}

impl Drop for ArcBuffer {
    fn drop(&mut self) {
        self.block.drop::<()>();
    }
}

impl From<&[u8]> for ArcBuffer {
    fn from(slice: &[u8]) -> Self {
        let mut buffer = Self::new(slice.len());

        buffer.as_slice_mut().copy_from_slice(slice);

        buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn drop<T>(_: T) {}

    #[test]
    fn create_buffer() {
        ArcBuffer::new(42);
    }

    #[test]
    fn create_empty() {
        ArcBuffer::new(0);
    }

    #[test]
    fn create_zero_sized_buffer() {
        ArcBuffer::new(0);
    }

    #[test]
    fn clone() {
        let arc = ArcBuffer::new(42);
        drop(arc.clone());
        drop(arc);
    }

    #[test]
    fn convert_from_vec() {
        let _ = ArcBuffer::from(vec![0u8; 42].as_slice());
    }

    #[test]
    fn convert_from_string() {
        let _ = ArcBuffer::from("hello".as_bytes());
    }
}
