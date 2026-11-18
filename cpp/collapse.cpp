#include "collapse.h"

using namespace litehtml;

namespace cg
{
void collapse(const element::ptr& el)
{
    css_properties& css = el->css();
    css_margins     p   = css.get_padding();
    p.top.set_value(0, css_units_px);
    p.bottom.set_value(0, css_units_px);
    css.set_padding(p);
    css_borders b = css.get_borders();
    for(css_border* e : {&b.top, &b.bottom, &b.left, &b.right})
    {
        e->style = border_style_none;
        e->width = css_length(0, css_units_px);
    }
    css.set_borders(b);
    css.set_height(css_length::predef_value(sizing_auto));
    css.set_min_height(css_length(0, css_units_px));
    for(auto& child : el->children()) collapse(child);
}
} // namespace cg
