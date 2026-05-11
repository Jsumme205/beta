#![no_std]

use core::{alloc::Layout, fmt, ops::DerefMut, ptr::NonNull};

pub mod collections;

pub unsafe trait Allocator {
    unsafe fn allocate(&self, layout: Layout) -> NonNull<[u8]> {
        match unsafe { self.try_allocate(layout) } {
            Ok(ptr) => ptr,
            Err(err) => panic!("{err:?}"),
        }
    }

    unsafe fn try_allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError>;

    unsafe fn reallocate(
        &self,
        old: NonNull<u8>,
        old_layout: Layout,
        new_size: usize,
    ) -> NonNull<[u8]> {
        match unsafe { self.try_reallocate(old, old_layout, new_size) } {
            Ok(ptr) => ptr,
            Err(err) => panic!("{err:?}"),
        }
    }

    unsafe fn try_reallocate(
        &self,
        old: NonNull<u8>,
        old_layout: Layout,
        new_size: usize,
    ) -> Result<NonNull<[u8]>, AllocError>;

    unsafe fn deallocate(&self, layout: Layout, ptr: NonNull<u8>);
}

unsafe impl<A> Allocator for &A
where
    A: Allocator + ?Sized,
{
    unsafe fn allocate(&self, layout: Layout) -> NonNull<[u8]> {
        unsafe { (*self).allocate(layout) }
    }

    unsafe fn deallocate(&self, layout: Layout, ptr: NonNull<u8>) {
        unsafe {
            (*self).deallocate(layout, ptr);
        }
    }

    unsafe fn reallocate(
        &self,
        old: NonNull<u8>,
        old_layout: Layout,
        new_size: usize,
    ) -> NonNull<[u8]> {
        unsafe { (*self).reallocate(old, old_layout, new_size) }
    }

    unsafe fn try_allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        unsafe { (*self).try_allocate(layout) }
    }

    unsafe fn try_reallocate(
        &self,
        old: NonNull<u8>,
        old_layout: Layout,
        new_size: usize,
    ) -> Result<NonNull<[u8]>, AllocError> {
        unsafe { (*self).try_reallocate(old, old_layout, new_size) }
    }
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
pub enum AllocError {
    OutOfMemory { layout: Layout },
    AlignTooLarge { layout: Layout },
    SizeTooLarge { layout: Layout },
    Other(&'static str),
}

impl fmt::Display for AllocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AllocError::OutOfMemory { layout } => {
                write!(f, "out of memory error for layout: {layout:?}")
            }
            AllocError::AlignTooLarge { layout } => {
                write!(f, "the requested alignment is too large {layout:?}")
            }
            AllocError::SizeTooLarge { layout } => {
                write!(f, "the requested size was too large: {layout:?}")
            }
            AllocError::Other(description) => write!(f, "an other error happened: {description}"),

            #[allow(unreachable_patterns)]
            _ => Ok(()),
        }
    }
}

impl core::error::Error for AllocError {}

pub unsafe trait Deinit<A> {
    unsafe fn deinit(&mut self, allocator: &A);
}

pub struct DeinitOnDrop<'a, T: Deinit<A>, A> {
    alloc: &'a A,
    value: T,
}

impl<T, A> core::ops::Deref for DeinitOnDrop<'_, T, A>
where
    T: Deinit<A>,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T, A> core::ops::DerefMut for DeinitOnDrop<'_, T, A>
where
    T: Deinit<A>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<T, A> Drop for DeinitOnDrop<'_, T, A>
where
    T: Deinit<A>,
{
    fn drop(&mut self) {
        unsafe {
            self.value.deinit(self.alloc);
        }
    }
}
