#include "cell_gaps.h"

#include "dom.h"
#include "geometry.h"
#include "superscripts.h"
#include "text.h"

#include <string>

using namespace litehtml;

namespace cg
{
namespace
{
bool cell(const element::ptr& el) { return el->css().get_display() == display_table_cell; }

bool worded(const element::ptr& el)
{
    std::string text;
    shown_text(el, text);
    return !squeezed(text).empty();
}

// A length in px; one in another unit, a share or an em, sets the cells
// apart as far as a column does.
float px(const css_length& l) { return px_or(l, l.val() == 0 ? 0 : CW); }

// The px between two cells side by side: padding, borders and spacing.
float between(const element::ptr& left, const element::ptr& right)
{
    const css_properties &l = left->css(), &r = right->css();
    float gap = px(l.get_padding().right) + px(r.get_padding().left) + px(l.get_borders().right.width) +
                px(r.get_borders().left.width);
    if(element::ptr table = table_of(left)) gap += px(table->css().get_border_spacing_x());
    return gap;
}
} // namespace

void gap_cells(const element::ptr& root)
{
    element::ptr before;
    for(auto& child : root->children())
    {
        if(child->is_comment() || child->css().get_display() == display_none) continue;
        if(cell(child))
        {
            // Raised digits stay on the words before them: cents on a price.
            if(before && worded(before) && worded(child) && !raised(child) && between(before, child) < CW)
            {
                // A whole column: less would snap away with the padding.
                css_margins p = before->css().get_padding();
                p.right.set_value(CW, css_units_px);
                before->css().set_padding(p);
            }
            before = child;
        }
        gap_cells(child);
    }
}
} // namespace cg
