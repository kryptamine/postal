// Table cells a mobile style turns into table groups, set as blocks.
#ifndef CG_REGROUPED_CELLS_H
#define CG_REGROUPED_CELLS_H

#include <litehtml.h>

namespace cg
{
// Before layout: a `td` or `th` under `root` shown as a table group or a
// row is a block. Mail reorders stacked cells that way, and litehtml drew
// such a cell's words over each other.
void block_regrouped_cells(const litehtml::element::ptr& root);
} // namespace cg

#endif
