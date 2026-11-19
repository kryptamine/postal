// Table columns stacked into rows, where they run past the pane.
#ifndef CG_STACKING_H
#define CG_STACKING_H

#include <litehtml.h>

namespace cg
{
// Every table round a box that ends past `width` px becomes blocks, one
// cell under another, as a narrow screen shows a mail's columns. False
// when there was no such table.
bool stack_spilt_tables(const litehtml::element::ptr& root, float width);
} // namespace cg

#endif
