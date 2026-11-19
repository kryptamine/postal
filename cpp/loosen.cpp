#include "loosen.h"

#include "dom.h"

using namespace litehtml;

namespace cg
{
namespace
{
bool fixed(const css_length& l) { return !l.is_predefined() && l.units() != css_units_percentage; }
} // namespace

void loosen(const element::ptr& root)
{
    css_properties& css = root->css();
    css.set_min_width(css_length(0, css_units_px));
    // An image's width is what its label is cut to.
    if(fixed(css.get_width()) && !tag_is(root, "img")) css.set_width(css_length::predef_value(sizing_auto));
    if(css.get_white_space() == white_space_nowrap) css.set_white_space(white_space_normal);
    if(css.get_white_space() == white_space_pre) css.set_white_space(white_space_pre_wrap);

    for(auto& child : root->children()) loosen(child);
}
} // namespace cg
