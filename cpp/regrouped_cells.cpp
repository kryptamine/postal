#include "regrouped_cells.h"

#include "dom.h"

using namespace litehtml;

namespace cg
{
void block_regrouped_cells(const element::ptr& root)
{
    style_display d = root->css().get_display();
    bool group = d == display_table_header_group || d == display_table_footer_group || d == display_table_row_group ||
                 d == display_table_row;
    if(group && (tag_is(root, "td") || tag_is(root, "th"))) root->css().set_display(display_block);
    for(auto& child : root->children()) block_regrouped_cells(child);
}
} // namespace cg
