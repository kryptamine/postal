// Table rows as high as their cells ask.
#ifndef CG_ROW_HEIGHTS_H
#define CG_ROW_HEIGHTS_H

#include <litehtml.h>

namespace cg
{
// Before layout: each row under `root` is as high as the highest of its
// cells. litehtml reads only the row's own height, and a spacer cell of
// `height="32"` took no room.
void rows_as_high_as_cells(const litehtml::element::ptr& root);
} // namespace cg

#endif
