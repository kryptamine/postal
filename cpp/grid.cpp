#include "grid.h"

#include "geometry.h"

#include <algorithm>
#include <cstring>

namespace cg
{
Box cells_of(const litehtml::position& p)
{
    Box box;
    span(p.y, p.height, CH, box.r0, box.r1);
    span(p.x, p.width, CW, box.c0, box.c1);
    return box;
}

bool is(const cg_cell& c, const char* glyph) { return std::strcmp(c.glyph, glyph) == 0; }

void set_glyph(cg_cell& c, const char* g, size_t n)
{
    n = std::min(n, sizeof c.glyph - 1);
    std::memcpy(c.glyph, g, n);
    c.glyph[n] = 0;
}

bool centre_in(int row, int col, const litehtml::position& p)
{
    float x = (float(col) + 0.5f) * CW, y = (float(row) + 0.5f) * CH;
    float px = p.x, py = p.y, pw = p.width, ph = p.height;
    return x >= px && x < px + pw && y >= py && y < py + ph;
}

bool rounded(litehtml::pixel_t x, litehtml::pixel_t y) { return float(x) >= 4 && float(y) >= 4; }

cg_cell* Grid::at(int row, int col)
{
    if(row < 0 || col < 0 || col >= cols) return nullptr;
    if(row >= rows())
    {
        cg_cell blank{};
        blank.glyph[0] = ' ';
        drawn.resize(size_t(row + 1) * size_t(cols), blank);
    }

    return &drawn[size_t(row) * size_t(cols) + size_t(col)];
}
} // namespace cg
