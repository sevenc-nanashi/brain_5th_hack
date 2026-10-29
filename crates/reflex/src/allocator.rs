//! Tracked system allocations for the single-core PEG execution environment.
//! Do not allocate from interrupt handlers or use this on multiple cores.
use core::{
    alloc::{GlobalAlloc, Layout},
    cell::UnsafeCell,
    ffi::c_void,
    ptr,
};

struct Header {
    original: *mut c_void,
    previous: *mut Header,
    next: *mut Header,
}

pub struct SystemAllocator {
    head: UnsafeCell<*mut Header>,
}

// List changes are protected against interruption on the supported single core.
unsafe impl Sync for SystemAllocator {}

impl Default for SystemAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemAllocator {
    pub const fn new() -> Self {
        Self {
            head: UnsafeCell::new(ptr::null_mut()),
        }
    }

    /// Reclaim allocations left after all application owners have been dropped.
    ///
    /// # Safety
    /// No live references, future drops, callbacks, or concurrent users may access
    /// any tracked allocation. Does not run destructors. Never call during panic.
    pub unsafe fn release_all(&self) {
        unsafe {
            let mut header = crate::runtime::critical_section(|| {
                let head = *self.head.get();
                *self.head.get() = ptr::null_mut();
                head
            });
            while !header.is_null() {
                let next = (*header).next;
                crate::runtime::free((*header).original);
                header = next;
            }
        }
    }
}

unsafe impl GlobalAlloc for SystemAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let alignment = layout.align().max(align_of::<Header>());
        let Some(size) = layout
            .size()
            .checked_add(size_of::<Header>())
            .and_then(|size| size.checked_add(alignment - 1))
            .filter(|size| *size <= isize::MAX as usize)
        else {
            return ptr::null_mut();
        };
        unsafe {
            let original = crate::runtime::alloc(size);
            if original.is_null() {
                return ptr::null_mut();
            }
            let start = original.cast::<u8>().add(size_of::<Header>());
            let offset = start.align_offset(alignment);
            let pointer = start.add(offset);
            let header = pointer.cast::<Header>().sub(1);
            crate::runtime::critical_section(|| {
                header.write(Header {
                    original,
                    previous: ptr::null_mut(),
                    next: *self.head.get(),
                });
                if !(*self.head.get()).is_null() {
                    (**self.head.get()).previous = header;
                }
                *self.head.get() = header;
            });
            pointer
        }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, _: Layout) {
        unsafe {
            let header = pointer.cast::<Header>().sub(1);
            let original = crate::runtime::critical_section(|| {
                let Header {
                    original,
                    previous,
                    next,
                } = header.read();
                if previous.is_null() {
                    *self.head.get() = next;
                } else {
                    (*previous).next = next;
                }
                if !next.is_null() {
                    (*next).previous = previous;
                }
                original
            });
            crate::runtime::free(original);
        }
    }
}
