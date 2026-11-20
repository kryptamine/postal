// The C ABI between the litehtml cell-grid container and Rust (src/cell_grid.rs).
#ifndef CELL_GRID_H
#define CELL_GRID_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

enum
{
    CG_BOLD = 1,
    CG_ITALIC = 2,
    CG_UNDERLINE = 4,
    CG_TEXT = 8,
    // The right half of a wide glyph, which prints nothing.
    CG_WIDE_TAIL = 16,
    CG_BORDER = 32,
    // An image's alt text, drawn in its place.
    CG_ALT = 64,
    // A border of the box round the whole mail, which only frames the page.
    CG_PAGE_BOX = 128,
    // The small print: text set smaller than the body, under its last words.
    CG_SMALL = 256,
    // The outer cell at the left or the right of a rounded button's fill.
    CG_CAP_LEFT = 512,
    CG_CAP_RIGHT = 1024,
};

// A colour is 0x01RRGGBB when set and 0 when not. A `bg` also carries
// CG_BG_PANEL when it comes from a box too large to be a control: a section
// or the page, which may give way to the terminal's own. `link` is 0 or an id.
#define CG_BG_PANEL 0x02000000u
// In a `bg`: the box filled has rounded corners.
#define CG_BG_ROUND 0x04000000u

// How src/mail_colours.rs moves mail colours, which the layout must know:
// a background over this WCAG luminance is the page and gives way to the
// terminal's, and a colour whose channels spread less than this is a grey.
#define CG_PAGE_LUMINANCE 0.6f
#define CG_GREY_SPREAD 40
typedef struct cg_cell
{
    char glyph[16];
    uint32_t fg;
    uint32_t bg;
    uint32_t flags;
    uint32_t link;
} cg_cell;

typedef struct cg_grid cg_grid;

// Lays `html` out `cols` cells wide and draws it; NULL when litehtml fails.
cg_grid* cg_render(const char* html, size_t len, int32_t cols);
void cg_free(cg_grid* grid);
// `rows` rows of the `cols` given to cg_render, row by row.
const cg_cell* cg_cells(const cg_grid* grid, size_t* rows);
// Whether the layout fits the cols it was given, or loses what is past them.
int32_t cg_fits(const cg_grid* grid);
// The href of link id `link`, or NULL past the last one.
const char* cg_href(const cg_grid* grid, uint32_t link);

#ifdef __cplusplus
}
#endif

#endif
