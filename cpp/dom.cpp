#include "dom.h"

#include <cstring>

using namespace litehtml;

namespace cg
{
bool tag_is(const element::ptr& el, const char* tag) { return std::strcmp(el->get_tagName(), tag) == 0; }

void shown_text(const element::ptr& el, std::string& out)
{
    if(el->is_comment() || el->css().get_display() == display_none) return;
    if(el->is_text() || el->is_space()) el->get_text(out);
    for(auto& child : el->children()) shown_text(child, out);
}

element::ptr link_of(const element::ptr& el)
{
    for(auto p = el->parent(); p; p = p->parent())
        if(tag_is(p, "a"))
            if(const char* href = p->get_attr("href"); href && *href) return p;
    return nullptr;
}
} // namespace cg
