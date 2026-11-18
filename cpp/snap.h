// Computed styles snapped to whole cells before layout.
#ifndef CG_SNAP_H
#define CG_SNAP_H

#include <litehtml.h>

namespace cg
{
// Box edges snap to whole cells before layout, so a separator gets a row of
// its own and a button's label sits in the middle of its block.
void snap_tree(const litehtml::element::ptr& root);
} // namespace cg

#endif
