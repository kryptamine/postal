//! litehtml through the C++ container in `cpp/cell_grid.cpp`. Its memory
//! is not handed back to the system, so it runs in a process of its own:
//! see docs/adr/0006-rendered-view-engine.md.

use std::{
    ffi::{CStr, c_char},
    ptr::NonNull,
};

use crate::render::cell_grid::{Cell, CellGrid};

#[repr(C)]
struct RawGrid {
    _opaque: [u8; 0],
}

unsafe extern "C" {
    fn cg_render(html: *const c_char, len: usize, cols: i32) -> *mut RawGrid;
    fn cg_free(grid: *mut RawGrid);
    fn cg_cells(grid: *const RawGrid, rows: *mut usize) -> *const Cell;
    fn cg_fits(grid: *const RawGrid) -> i32;
    fn cg_href(grid: *const RawGrid, link: u32) -> *const c_char;
}

/// The container's grid, freed on drop.
struct Owned(NonNull<RawGrid>);

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: the pointer came from `cg_render` and is freed once, here.
        unsafe { cg_free(self.0.as_ptr()) }
    }
}

/// `html` laid out `cols` cells wide; `None` when litehtml fails on it.
pub fn lay_out(html: &str, cols: u16) -> Option<CellGrid> {
    // SAFETY: the pointer and length describe `html`, which outlives the
    // call; the container copies what it keeps.
    let raw = unsafe { cg_render(html.as_ptr().cast(), html.len(), i32::from(cols)) };
    let grid = Owned(NonNull::new(raw)?);
    let mut rows = 0;
    // SAFETY: the grid is live until `grid` drops, and never changes.
    let cells = unsafe { cg_cells(grid.0.as_ptr(), &raw mut rows) };
    let len = rows * usize::from(cols);
    let cells = if cells.is_null() || len == 0 {
        Vec::new()
    } else {
        // SAFETY: the container holds `rows * cols` contiguous cells.
        unsafe { std::slice::from_raw_parts(cells, len) }.to_vec()
    };

    let hrefs = (1..)
        .map_while(|link| {
            // SAFETY: the grid is live; past the last link the answer is null.
            let href = unsafe { cg_href(grid.0.as_ptr(), link) };
            // SAFETY: a non-null href is a NUL-terminated string the grid owns.
            (!href.is_null()).then(|| unsafe { CStr::from_ptr(href) })
        })
        .map(|href| href.to_string_lossy().into_owned())
        .collect();

    // SAFETY: the grid is live until `grid` drops.
    let fits = unsafe { cg_fits(grid.0.as_ptr()) } != 0;
    Some(CellGrid::new(cols, cells, hrefs, fits))
}
