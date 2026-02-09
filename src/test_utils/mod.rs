macro_rules! test_suite {
	($make_allocator:expr, $make_small_allocator:expr) => {
    extern crate std;
    use core::alloc::{GlobalAlloc, Layout};
    use std::boxed::Box;
    use std::string::String;
    use std::sync::Arc;
    use std::vec::Vec;


    // ========================================
    // Basic sanity
    // ========================================

    #[test]
    fn test_basic_alloc_dealloc() {
        let allocator = $make_allocator;

        unsafe {
            let layout = Layout::from_size_align(64, 8).unwrap();
            let ptr = allocator.alloc(layout);
            assert!(!ptr.is_null());

            // write to the memory to make sure it's usable
            ptr.write_bytes(0xAB, 64);

            allocator.dealloc(ptr, layout);
        }
    }

    #[test]
    fn test_zero_size() {
        let allocator = $make_allocator;

        unsafe {
            // Layout::from_size_align allows size 0
            let layout = Layout::from_size_align(0, 1).unwrap();
            let ptr = allocator.alloc(layout);
            // Behavior for zero-size is implementation-defined,
            // but it should not panic or return an invalid pointer
            if !ptr.is_null() {
                allocator.dealloc(ptr, layout);
            }
        }
    }

    #[test]
    fn test_single_byte() {
        let allocator = $make_allocator;

        unsafe {
            let layout = Layout::from_size_align(1, 1).unwrap();
            let ptr = allocator.alloc(layout);
            assert!(!ptr.is_null());
            *ptr = 0xFF;
            assert_eq!(*ptr, 0xFF);
            allocator.dealloc(ptr, layout);
        }
    }

    // ========================================
    // Alignment correctness
    // ========================================

    #[test]
    fn test_alignment() {
        let allocator = $make_allocator;

        unsafe {
            for align in [1, 2, 4, 8, 16, 32, 64, 128, 256] {
                let layout = Layout::from_size_align(align, align).unwrap();
                let ptr = allocator.alloc(layout);
                assert!(!ptr.is_null());
                assert_eq!(
                    ptr as usize % align,
                    0,
                    "pointer {ptr:?} not aligned to {align}"
                );
                allocator.dealloc(ptr, layout);
            }
        }
    }

    #[test]
    fn test_large_alignment_small_size() {
        let allocator = $make_allocator;

        unsafe {
            // 4 bytes but needs 128-byte alignment
            let layout = Layout::from_size_align(4, 128).unwrap();
            let ptr = allocator.alloc(layout);
            assert!(!ptr.is_null());
            assert_eq!(ptr as usize % 128, 0);
            allocator.dealloc(ptr, layout);
        }
    }

    #[test]
    fn test_many_different_alignments_at_once() {
        let allocator = $make_allocator;
        let mut ptrs = Vec::new();

        unsafe {
            for align in [1, 2, 4, 8, 16, 32, 64] {
                let layout = Layout::from_size_align(32, align).unwrap();
                let ptr = allocator.alloc(layout);
                assert!(!ptr.is_null());
                assert_eq!(
                    ptr as usize % align,
                    0,
                    "pointer {ptr:?} not aligned to {align}"
                );
                ptrs.push((ptr, layout));
            }

            for (ptr, layout) in ptrs {
                allocator.dealloc(ptr, layout);
            }
        }
    }

    // ========================================
    // Overlap detection
    // ========================================

    #[test]
    fn test_no_overlap() {
        let allocator = $make_allocator;

        unsafe {
            let layout = Layout::from_size_align(128, 8).unwrap();
            let a = allocator.alloc(layout);
            let b = allocator.alloc(layout);
            let c = allocator.alloc(layout);

            assert!(!a.is_null());
            assert!(!b.is_null());
            assert!(!c.is_null());

            // regions must not overlap
            let a_range = a as usize..a as usize + 128;
            let b_range = b as usize..b as usize + 128;

            assert!(!a_range.contains(&(b as usize)), "a and b overlap");
            assert!(!a_range.contains(&(c as usize)), "a and c overlap");
            assert!(!b_range.contains(&(c as usize)), "b and c overlap");

            // write different patterns and verify they don't corrupt each other
            a.write_bytes(0xAA, 128);
            b.write_bytes(0xBB, 128);
            c.write_bytes(0xCC, 128);

            assert!(
                std::slice::from_raw_parts(a, 128)
                    .iter()
                    .all(|&x| x == 0xAA)
            );
            assert!(
                std::slice::from_raw_parts(b, 128)
                    .iter()
                    .all(|&x| x == 0xBB)
            );
            assert!(
                std::slice::from_raw_parts(c, 128)
                    .iter()
                    .all(|&x| x == 0xCC)
            );

            allocator.dealloc(a, layout);
            allocator.dealloc(b, layout);
            allocator.dealloc(c, layout);
        }
    }

    // ========================================
    // Reuse after free
    // ========================================

    #[test]
    fn test_reuse_after_free() {
        let allocator = $make_allocator;

        unsafe {
            let layout = Layout::from_size_align(64, 8).unwrap();

            let a = allocator.alloc(layout);
            assert!(!a.is_null());
            allocator.dealloc(a, layout);

            // allocator should reuse the freed block
            let b = allocator.alloc(layout);
            assert!(!b.is_null());

            allocator.dealloc(b, layout);
        }
    }

    #[test]
    fn test_alloc_free_alloc_cycle() {
        let allocator = $make_allocator;

        unsafe {
            let layout = Layout::from_size_align(128, 8).unwrap();

            for _ in 0..100 {
                let ptr = allocator.alloc(layout);
                assert!(!ptr.is_null());
                ptr.write_bytes(0xAB, 128);
                allocator.dealloc(ptr, layout);
            }
        }
    }

    // ========================================
    // Out of memory
    // ========================================

    #[test]
    fn test_oom() {
        let allocator = $make_small_allocator;

        unsafe {
            let layout = Layout::from_size_align(512, 8).unwrap();
            let ptr = allocator.alloc(layout);
            assert!(ptr.is_null(), "should fail gracefully on OOM");
        }
    }

    #[test]
    fn test_exhaust_heap() {
        let allocator = $make_small_allocator;
        let layout = Layout::from_size_align(64, 8).unwrap();
        let mut ptrs = Vec::new();

        unsafe {
            loop {
                let ptr = allocator.alloc(layout);
                if ptr.is_null() {
                    break;
                }
                ptrs.push(ptr);
            }

            // we got some allocations before running out
            assert!(!ptrs.is_empty(), "should have allocated at least once");

            // free everything
            for ptr in ptrs {
                allocator.dealloc(ptr, layout);
            }
        }
    }

    // ========================================
    // Coalescing / fragmentation
    // ========================================

    #[test]
    fn test_coalescing() {
        let allocator = $make_allocator;
        let layout = Layout::from_size_align(256, 8).unwrap();

        unsafe {
            // fill with adjacent blocks
            let a = allocator.alloc(layout);
            let b = allocator.alloc(layout);
            let c = allocator.alloc(layout);
            assert!(!a.is_null());
            assert!(!b.is_null());
            assert!(!c.is_null());

            // free all three — they should coalesce
            allocator.dealloc(a, layout);
            allocator.dealloc(b, layout);
            allocator.dealloc(c, layout);

            // now a large allocation should succeed
            let big_layout = Layout::from_size_align(768, 8).unwrap();
            let big = allocator.alloc(big_layout);
            assert!(
                !big.is_null(),
                "coalescing failed — couldn't allocate merged block"
            );

            allocator.dealloc(big, big_layout);
        }
    }

    #[test]
    fn test_free_in_reverse_order() {
        let allocator = $make_allocator;
        let layout = Layout::from_size_align(64, 8).unwrap();

        unsafe {
            let a = allocator.alloc(layout);
            let b = allocator.alloc(layout);
            let c = allocator.alloc(layout);

            // free in reverse order
            allocator.dealloc(c, layout);
            allocator.dealloc(b, layout);
            allocator.dealloc(a, layout);

            // should still be able to allocate a large block
            let big_layout = Layout::from_size_align(192, 8).unwrap();
            let big = allocator.alloc(big_layout);
            assert!(!big.is_null(), "reverse-order free coalescing failed");
            allocator.dealloc(big, big_layout);
        }
    }

    #[test]
    fn test_free_middle_block() {
        let allocator = $make_allocator;
        let layout = Layout::from_size_align(64, 8).unwrap();

        unsafe {
            let a = allocator.alloc(layout);
            let b = allocator.alloc(layout);
            let c = allocator.alloc(layout);

            // free only the middle block
            allocator.dealloc(b, layout);

            // allocate something that fits in the freed block
            let small_layout = Layout::from_size_align(32, 8).unwrap();
            let d = allocator.alloc(small_layout);
            assert!(!d.is_null());

            allocator.dealloc(a, layout);
            allocator.dealloc(c, layout);
            allocator.dealloc(d, small_layout);
        }
    }

    // ========================================
    // Mixed sizes
    // ========================================

    #[test]
    fn test_mixed_sizes() {
        let allocator = $make_allocator;
        let mut ptrs = Vec::new();

        unsafe {
            let sizes = [8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096];

            for &size in &sizes {
                let layout = Layout::from_size_align(size, 8).unwrap();
                let ptr = allocator.alloc(layout);
                assert!(!ptr.is_null(), "failed to alloc {size} bytes");
                ptr.write_bytes(0xAB, size);
                ptrs.push((ptr, layout));
            }

            // free every other one
            let mut i = 0;
            ptrs.retain(|(ptr, layout)| {
                i += 1;
                if i % 2 == 0 {
                    allocator.dealloc(*ptr, *layout);
                    false
                } else {
                    true
                }
            });

            // allocate more
            for &size in &[24, 48, 96, 200] {
                let layout = Layout::from_size_align(size, 8).unwrap();
                let ptr = allocator.alloc(layout);
                assert!(
                    !ptr.is_null(),
                    "failed to alloc {size} bytes after partial free"
                );
                ptrs.push((ptr, layout));
            }

            // cleanup
            for (ptr, layout) in ptrs {
                allocator.dealloc(ptr, layout);
            }
        }
    }

    // ========================================
    // Stress test with random sizes
    // ========================================

    #[test]
    fn test_random_workload() {
        let allocator = $make_allocator;
        let mut live: Vec<(*mut u8, Layout)> = Vec::new();

        // Simple deterministic RNG using hashing (no external dependency)
        let mut seed: u64 = 12345;
        let mut next_rand = || -> u64 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };

        for _ in 0..10_000 {
            let should_alloc = live.is_empty() || (next_rand() % 10) < 6;

            if should_alloc {
                let size = (next_rand() % 1024 + 1) as usize;
                let align_shift = (next_rand() % 8) as usize; // 0..7
                let align = 1usize << align_shift; // 1, 2, 4, ..., 128
                let layout = Layout::from_size_align(size, align).unwrap();

                unsafe {
                    let ptr = allocator.alloc(layout);
                    if !ptr.is_null() {
                        // touch the memory
                        ptr.write_bytes(0xAB, size);
                        assert_eq!(
                            ptr as usize % align,
                            0,
                            "misaligned: {ptr:?} % {align} != 0"
                        );
                        live.push((ptr, layout));
                    }
                }
            } else {
                // free a random allocation
                let idx = (next_rand() as usize) % live.len();
                let (ptr, layout) = live.swap_remove(idx);
                unsafe {
                    allocator.dealloc(ptr, layout);
                }
            }
        }

        // cleanup remaining
        for (ptr, layout) in live {
            unsafe {
                allocator.dealloc(ptr, layout);
            }
        }
    }

    // ========================================
    // Thread safety
    // ========================================

    #[test]
    fn test_concurrent() {
        let allocator = Arc::new($make_allocator);

        let handles: Vec<_> = (0..8)
            .map(|_| {
                let alloc = allocator.clone();
                std::thread::spawn(move || {
                    let layout = Layout::from_size_align(64, 8).unwrap();
                    for _ in 0..10 {
                        unsafe {
                            let ptr = alloc.alloc(layout);
                            assert!(!ptr.is_null());
                            ptr.write_bytes(0xFF, 64);
                            alloc.dealloc(ptr, layout);
                        }
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }
    }

    #[test]
    fn test_concurrent_mixed_sizes() {
        let allocator = Arc::new($make_allocator);

        let handles: Vec<_> = (0..8)
            .map(|thread_id| {
                let alloc = allocator.clone();
                std::thread::spawn(move || {
                    let mut ptrs = Vec::new();

                    for i in 0..500 {
                        let size = ((thread_id * 37 + i * 13) % 512 + 1) as usize;
                        let layout = Layout::from_size_align(size, 8).unwrap();

                        unsafe {
                            let ptr = alloc.alloc(layout);
                            if !ptr.is_null() {
                                ptr.write_bytes(thread_id as u8, size);
                                ptrs.push((ptr, layout, size, thread_id as u8));
                            }
                        }

                        // periodically free some
                        if ptrs.len() > 10 && i % 3 == 0 {
                            let (ptr, layout, _, _) = ptrs.pop().unwrap();
                            unsafe { alloc.dealloc(ptr, layout) };
                        }
                    }

                    // verify no corruption
                    for &(ptr, _, size, tag) in &ptrs {
                        let slice = unsafe { std::slice::from_raw_parts(ptr, size) };
                        assert!(
                            slice.iter().all(|&b| b == tag),
                            "memory corruption detected in thread {thread_id}"
                        );
                    }

                    // cleanup
                    for (ptr, layout, _, _) in ptrs {
                        unsafe { alloc.dealloc(ptr, layout) };
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }
    }

    // ========================================
    // Use as global allocator
    // ========================================

    // Uncomment to test as the actual global allocator:
    //
    // #[global_allocator]
    // static ALLOCATOR: MyAllocator = MyAllocator::new();

    #[test]
    fn test_as_global_with_vec() {
        // If using #[global_allocator], this exercises your allocator
        // through real Rust collections
        let v: Vec<i32> = (0..1000).collect();
        assert_eq!(v.len(), 1000);
        assert_eq!(v[999], 999);
    }

    #[test]
    fn test_as_global_with_string() {
        let s = String::from("hello world, this is a string that will trigger allocation");
        assert!(s.contains("hello"));

        let mut s2 = String::new();
        for i in 0..100 {
            s2.push_str(&std::format!("item {i} "));
        }
        assert!(s2.contains("item 99"));
    }

    #[test]
    fn test_as_global_with_box() {
        let b = Box::new([0u8; 4096]);
        assert_eq!(b[0], 0);
        assert_eq!(b[4095], 0);
    }

    #[test]
    fn test_as_global_vec_grow_shrink() {
        let mut v = Vec::new();

        // grow
        for i in 0..10_000 {
            v.push(i);
        }
        assert_eq!(v.len(), 10_000);

        // shrink
        v.truncate(100);
        v.shrink_to_fit();
        assert_eq!(v.len(), 100);

        // grow again
        for i in 0..10_000 {
            v.push(i);
        }
        assert_eq!(v.len(), 10_100);
    }
}

}
