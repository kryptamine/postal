#include "stacking.h"

#include <vector>

using namespace litehtml;

namespace cg
{
namespace
{
bool table(style_display d) { return d == display_table || d == display_inline_table; }

bool table_part(style_display d)
{
    return table(d) || d == display_table_row_group || d == display_table_header_group ||
           d == display_table_footer_group || d == display_table_row || d == display_table_cell;
}

// A table nested in a cell is left to its own turn.
void as_blocks(const element::ptr& el, bool top)
{
    style_display d = el->css().get_display();
    if(!table_part(d) || (table(d) && !top)) return;
    el->css().set_display(display_block);
    el->css().set_width(css_length::predef_value(sizing_auto));
    for(auto& child : el->children()) as_blocks(child, false);
}

// Whether a box under `el` ends past `width`; the tables round one go in
// `out`. They change only after the walk, which reads where they put things.
bool spilt(const element::ptr& el, float width, std::vector<element::ptr>& out)
{
    if(el->is_comment() || el->css().get_display() == display_none) return false;
    bool past = false;
    if(el->children().empty() && el->get_render_item())
    {
        position p = el->get_placement();
        past = float(p.width) > 0 && float(p.x) + float(p.width) > width + 0.5f;
    }
    for(auto& child : el->children()) past = spilt(child, width, out) || past;
    if(past && table(el->css().get_display())) out.push_back(el);
    return past;
}
} // namespace

bool stack_spilt_tables(const element::ptr& root, float width)
{
    std::vector<element::ptr> tables;
    spilt(root, width, tables);
    for(auto& t : tables) as_blocks(t, true);
    return !tables.empty();
}
} // namespace cg
