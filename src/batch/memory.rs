use super::MIB;
use crate::tr;
use crate::{render, settings::Settings};
use anyhow::Result;
use image::ImageDecoder;
use std::path::Path;

/// Conservative working-set estimate, including full input decode and intermediate output buffers.
pub fn estimate_peak_bytes(path: &Path, settings: &Settings) -> Result<u64> {
    let decoder = image::ImageReader::open(path)?
        .with_guessed_format()?
        .into_decoder()?;
    let (w, h) = decoder.dimensions();
    // Decoding/orientation may retain both the source and a transformed buffer.
    let input_bytes = u64::from(decoder.color_type().bytes_per_pixel()).max(4) * 2;
    let pixels = u64::from(w) * u64::from(h);
    anyhow::ensure!(pixels <= 100_000_000, tr!("errors.input_limit"));
    let mut settings = settings.clone();
    settings.normalize();
    let output_pixels = [(w, h), (h, w)]
        .into_iter()
        .map(|(w, h)| {
            let initial = render::layout(w, h, &settings);
            let scale = if settings.max_edge == 0 {
                1.
            } else {
                (settings.max_edge as f64 / initial.width.max(initial.height) as f64).min(1.)
            };
            let layout = render::layout(
                (w as f64 * scale).round().max(1.) as u32,
                (h as f64 * scale).round().max(1.) as u32,
                &settings,
            );
            u64::from(layout.width) * u64::from(layout.height)
        })
        .max()
        .unwrap_or(0);
    Ok(pixels
        .saturating_mul(input_bytes)
        .saturating_add(output_pixels.saturating_mul(20))
        .saturating_add(64 * MIB))
}

#[cfg(windows)]
pub(super) fn available_memory() -> Option<u64> {
    #[repr(C)]
    struct MemoryStatus {
        length: u32,
        load: u32,
        total_phys: u64,
        avail_phys: u64,
        total_page: u64,
        avail_page: u64,
        total_virtual: u64,
        avail_virtual: u64,
        avail_extended: u64,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GlobalMemoryStatusEx(status: *mut MemoryStatus) -> i32;
    }
    let mut status = MemoryStatus {
        length: std::mem::size_of::<MemoryStatus>() as u32,
        load: 0,
        total_phys: 0,
        avail_phys: 0,
        total_page: 0,
        avail_page: 0,
        total_virtual: 0,
        avail_virtual: 0,
        avail_extended: 0,
    };
    // SAFETY: status has the documented MEMORYSTATUSEX layout and initializes its size field.
    if unsafe { GlobalMemoryStatusEx(&mut status) } != 0 {
        Some(status.avail_phys)
    } else {
        None
    }
}

#[cfg(windows)]
pub(super) fn process_memory() -> Option<u64> {
    #[repr(C)]
    #[derive(Default)]
    struct ProcessMemoryCounters {
        size: u32,
        page_fault_count: u32,
        peak_working_set: usize,
        working_set: usize,
        quota_peak_paged: usize,
        quota_paged: usize,
        quota_peak_nonpaged: usize,
        quota_nonpaged: usize,
        pagefile: usize,
        peak_pagefile: usize,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
        fn K32GetProcessMemoryInfo(
            process: *mut std::ffi::c_void,
            counters: *mut ProcessMemoryCounters,
            size: u32,
        ) -> i32;
    }
    let mut counters = ProcessMemoryCounters {
        size: std::mem::size_of::<ProcessMemoryCounters>() as u32,
        ..Default::default()
    };
    // SAFETY: the counter layout and size match PROCESS_MEMORY_COUNTERS. The current
    // process pseudo-handle is always valid and must not be closed.
    if unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.size) } != 0 {
        Some(counters.working_set as u64)
    } else {
        None
    }
}

#[cfg(not(windows))]
pub(super) fn process_memory() -> Option<u64> {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|text| {
            text.lines()
                .find(|line| line.starts_with("VmRSS:"))?
                .split_whitespace()
                .nth(1)?
                .parse::<u64>()
                .ok()
                .map(|kb| kb * 1024)
        })
}

#[cfg(not(windows))]
pub(super) fn available_memory() -> Option<u64> {
    std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|text| {
            text.lines()
                .find(|line| line.starts_with("MemAvailable:"))
                .and_then(|line| line.split_whitespace().nth(1))
                .and_then(|value| value.parse::<u64>().ok())
                .map(|kb| kb * 1024)
        })
}
