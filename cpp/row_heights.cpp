#include "row_heights.h"

#include "dom.h"

#include <algorithm>

using namespace litehtml;

namespace cg
{
void rows_as_high_as_cells(const element::ptr& root)
{
    if(root->css().get_display() == display_table_row)
    {
        float high = px_or(root->css().get_height(), 0);
        for(auto& cell : root->children())
            if(cell->css().get_display() == display_table_cell)
                high = std::max(high, px_or(cell->css().get_height(), 0));
        if(high > 0) root->css().set_height(css_length(high, css_units_px));
    }

    for(auto& child : root->children()) rows_as_high_as_cells(child);
}
} // namespace cg
