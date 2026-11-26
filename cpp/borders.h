// Borders drawn on the cell grid after text, with junctions and styles.
#ifndef CG_BORDERS_H
#define CG_BORDERS_H

#include "grid.h"

#include <litehtml.h>

#include <vector>

namespace cg
{
struct Frame
{
    Box box;
    litehtml::borders b;
    std::vector<litehtml::position> clips;
};

// Draws every recorded frame into `grid`, stepping horizontal edges off
// text, blending translucent colours and joining touching borders. True
// when a left or right edge fell past the pane.
bool apply_borders(Grid& grid, const std::vector<Frame>& frames, const Box& mail);
} // namespace cg

#endif
