#include "dom.h"

#include "hidden.h"

#include <cstring>

using namespace litehtml;

namespace cg
{
bool tag_is(const element::ptr& el, const char* tag) { return std::strcmp(el->get_tagName(), tag) == 0; }

float px_or(const css_length& l, float none)
{
    return !l.is_predefined() && l.units() == css_units_px ? float(l.val()) : none;
}

void shown_text(const element::ptr& el, std::string& out)
{
    if(unseen(el)) return;
    if(el->is_text() || el->is_space()) el->get_text(out);
    for(auto& child : el->children()) shown_text(child, out);
}

bool shown(const css_border& b)
{
    return b.width.val() > 0 && b.style != border_style_none && b.style != border_style_hidden;
}

bool shown(const border& b)
{
    return float(b.width) > 0.f && b.style != border_style_none && b.style != border_style_hidden;
}

element::ptr link_of(const element::ptr& el)
{
    for(auto p = el->parent(); p; p = p->parent())
        if(tag_is(p, "a"))
            if(const char* href = p->get_attr("href"); href && *href) return p;
    return nullptr;
}

element::ptr table_of(const element::ptr& el)
{
    auto p = el->parent();
    while(p && !tag_is(p, "table")) p = p->parent();
    return p;
}

void leaf_boxes(const element::ptr& el, std::vector<position>& out)
{
    if(el->children().empty())
    {
        if(el->get_render_item())
        {
            position p = el->get_placement();
            if(float(p.width) > 0 && float(p.height) > 0) out.push_back(p);
        }
        return;
    }
    for(auto& child : el->children()) leaf_boxes(child, out);
}
} // namespace cg
