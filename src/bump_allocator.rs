
use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ptr;
use core::sync::atomic::AtomicPtr;
use core::sync::atomic::Ordering;


#[derive(Debug)]
#[repr(align(16))]
pub struct BumpAllocator<const HEAP_SIZE: usize> {
    heap: UnsafeCell<[u8; HEAP_SIZE]>,
    next_free: AtomicPtr<u8>,
}

unsafe impl<const HEAP_SIZE: usize> Sync for BumpAllocator<HEAP_SIZE> {}

impl<const HEAP_SIZE: usize> BumpAllocator<HEAP_SIZE> {
    pub const fn new(array: [u8; HEAP_SIZE]) -> Self {
        Self {
            heap: UnsafeCell::new(array),
            next_free: AtomicPtr::new(ptr::null_mut()),
        }
    }

    fn heap_start(&self) -> *const u8 {
        self.heap.get().cast()
    }
}

fn align_up(ptr: *mut u8, alignment: usize) -> *mut u8 {
    let mask = alignment - 1;
    ((ptr.addr() + mask) & !mask) as *mut u8
}

unsafe impl<const HEAP_SIZE: usize> GlobalAlloc for BumpAllocator<HEAP_SIZE> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut allocated_block_start = ptr::null_mut();
        let next_free =
            self.next_free
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |next_free| {
                    let next_free = if next_free.is_null() {
                        self.heap_start().cast_mut()
                    } else {
                        next_free
                    };
                    let next_block_start = align_up(next_free, layout.align());
                    let block_end = unsafe { next_block_start.add(layout.size()) };
                    let heap_end = unsafe { self.heap_start().add(HEAP_SIZE) }.cast_mut();
                    if block_end > heap_end {
                        return None;
                    }
                    allocated_block_start = next_block_start;
                    Some(block_end)
                });
        if next_free.is_err() {
            return ptr::null_mut();
        }
        allocated_block_start
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

#[cfg(test)]
mod test {
    use super::*;

   test_suite!{
        BumpAllocator::new([0; 65536]),
        BumpAllocator::new([0; 256])
	}
}