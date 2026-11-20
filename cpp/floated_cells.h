// Table cells the mail floats, kept in their rows.
#ifndef CG_FLOATED_CELLS_H
#define CG_FLOATED_CELLS_H

#include <litehtml.h>

namespace cg
{
// Before layout: a floated `td` or `th` under `root` is a cell again. A
// browser takes it out of its row as a block, and litehtml lays that
// block over the row's first glyph.
void unfloat_cells(const litehtml::element::ptr& root);
} // namespace cg

#endif
