// Table cells whose words would touch the next cell's.
#ifndef CG_CELL_GAPS_H
#define CG_CELL_GAPS_H

#include <litehtml.h>

namespace cg
{
// Before layout: a cell with words beside a cell with words gets a column
// of padding on its right where nothing else sets them apart. In 600 px
// the table spread them; in 74 columns `Support` met `Unsubscribe`.
void gap_cells(const litehtml::element::ptr& root);
} // namespace cg

#endif
