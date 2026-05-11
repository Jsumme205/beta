use core::{num::NonZero, ptr::NonNull};

use crate::{Allocator, collections::Properties};

pub struct RawVecInner {
    ptr: NonNull<u8>,
    capacity: usize,
}

impl RawVecInner {
    pub const fn new(props: Properties) -> Self {
        Self {
            ptr: NonNull::without_provenance(unsafe {
                NonZero::new_unchecked(props.layout.align())
            }),
            capacity: 0,
        }
    }

    pub fn with_capacity(capacity: usize, allocator: &dyn Allocator, props: Properties) -> Self {
        let layout = (props.array_layout)(capacity).expect("");
        unsafe {
            Self {
                ptr: allocator.allocate(layout).cast(),
                capacity,
            }
        }
    }

    unsafe fn __grow(&mut self, props: Properties, allocator: &dyn Allocator) {}

    pub unsafe fn push(
        &mut self,
        props: Properties,
        allocator: &dyn Allocator,
        init_len: &mut usize,
        write: &mut (dyn FnMut(*mut ()) + '_),
    ) {
        if *init_len >= self.capacity {
            unsafe {
                self.__grow(props, allocator);
            }
        }

        let byte_offset = props.layout.size() * *init_len;

        let raw_ptr = unsafe { self.ptr.add(byte_offset) };

        write(raw_ptr.as_ptr() as *mut ());
        *init_len += 1;
    }
}
