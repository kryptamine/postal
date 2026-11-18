#include "titles.h"

#include "collapse.h"
#include "dom.h"

#include <vector>

using namespace litehtml;

namespace cg
{
namespace
{
// Two lines of a 600 px mail: the title and the line over or under it.
const int TITLE_CELLS = 150;

bool filled(const element::ptr& el) { return el->css().get_bg().m_color.alpha != 0; }

bool title(const element::ptr& el, const Holds& h)
{
    if(!filled(el) || tag_is(el, "body") || tag_is(el, "html")) return false;
    return h.headed && !h.anchored && !h.labelled && h.length <= TITLE_CELLS;
}

void shown_fills(const element::ptr& el, std::vector<element::ptr>& out)
{
    if(el->is_comment() || el->css().get_display() == display_none) return;
    if(filled(el)) out.push_back(el);
    for(auto& child : el->children()) shown_fills(child, out);
}

bool related(const element* a, const element* b)
{
    for(auto p = a; p; p = p->parent().get())
        if(p == b) return true;
    for(auto p = b; p; p = p->parent().get())
        if(p == a) return true;
    return false;
}

// A box whose colour fills another box beside it is a piece of a section
// the mail built from many boxes, or one of its cards.
bool alone(const element::ptr& el, const std::vector<element::ptr>& fills)
{
    const web_color& fill = el->css().get_bg().m_color;
    for(auto& other : fills)
        if(other->css().get_bg().m_color == fill && !related(el.get(), other.get())) return false;
    return true;
}

// The fill goes with everything in it; margins inside only spaced the
// title off the fill's edge.
void strip(const element::ptr& el, bool outer)
{
    css_properties& css = el->css();
    background      bg  = css.get_bg();
    bg.m_color          = web_color::transparent;
    css.set_bg(bg);
    if(!outer)
    {
        css_margins m = css.get_margins();
        m.top.set_value(0, css_units_px);
        m.bottom.set_value(0, css_units_px);
        css.set_margins(m);
    }
    for(auto& child : el->children()) strip(child, false);
}

void unframe(const element::ptr& el, const Contents& contents, const std::vector<element::ptr>& fills)
{
    if(title(el, contents.of(el.get())) && alone(el, fills))
    {
        collapse(el);
        return strip(el, true);
    }
    for(auto& child : el->children()) unframe(child, contents, fills);
}
} // namespace

void unframe_titles(const element::ptr& root, const Contents& contents)
{
    std::vector<element::ptr> fills;
    shown_fills(root, fills);
    unframe(root, contents, fills);
}
} // namespace cg
