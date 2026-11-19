// The cells a mail is drawn in, and the boxes of them CSS px land on.
#ifndef CG_GRID_H
#define CG_GRID_H

#include "cell_grid.h"

#include <litehtml.h>

#include <vector>

namespace cg
{
// Cells from row r0 to r1 and column c0 to c1; empty when r1 < r0.
struct Box
{
    int r0, r1, c0, c1;

    bool holds(const Box& in) const
    {
        return in.r0 <= in.r1 && r0 <= in.r0 && r1 >= in.r1 && c0 <= in.c0 && c1 >= in.c1;
    }
};

// The cells whose centre lies in `p`.
Box cells_of(const litehtml::position& p);

bool is(const cg_cell& c, const char* glyph);

class Grid
{
  public:
    int                  cols;
    std::vector<cg_cell> drawn;

    explicit Grid(int cols) : cols(cols) {}

    int rows() const { return cols ? int(drawn.size()) / cols : 0; }
    // The cell, with the grid grown down to its row; null outside the columns.
    cg_cell* at(int row, int col);
};
} // namespace cg

#endif
