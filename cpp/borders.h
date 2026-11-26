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
// text, blending translucent colours and joining touching borders. Sets
// `lost_border` when a left or right edge fell past the pane.
void apply_borders(Grid& grid, const std::vector<Frame>& frames, const Box& mail, bool& lost_border);
} // namespace cg

#endif
