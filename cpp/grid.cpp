#include "grid.h"

#include "geometry.h"

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
