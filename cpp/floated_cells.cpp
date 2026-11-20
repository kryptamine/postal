#include "floated_cells.h"

#include "dom.h"
#include "geometry.h"

using namespace litehtml;

namespace cg
{
void unfloat_cells(const element::ptr& root)
{
    css_properties& css = root->css();
    if(css.get_float() != float_none && (tag_is(root, "td") || tag_is(root, "th")))
    {
        // A float is as wide as its words, which then never wrap, and one
        // floated right stood off the cell before it.
        bool right = css.get_float() == float_right;
        css.set_white_space(white_space_nowrap);
        css.set_text_align(right ? text_align_right : text_align_left);
        css_margins p = css.get_padding();
        if(right && p.left.val() < CW) p.left.set_value(CW, css_units_px);
        css.set_padding(p);
        css.set_float(float_none);
        css.set_display(display_table_cell);
    }
    for(auto& child : root->children()) unfloat_cells(child);
}
} // namespace cg
