#![no_std]
#![deny(unsafe_code)]

#[cfg(ktest)]
mod tests {
    use ostd::prelude::*;

    #[ktest]
    fn boot_info_has_memory_regions() {
        let memory_regions = &ostd::boot::boot_info().memory_regions;
        assert!(!memory_regions.is_empty());
    }
}
