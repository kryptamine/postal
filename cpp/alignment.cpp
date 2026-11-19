#include "alignment.h"

#include "dom.h"

#include <set>

using namespace litehtml;

namespace cg
{
namespace
{
const size_t PARAGRAPH_LINES = 3;

bool in_flow(const element::ptr& el)
{
    style_display d = el->css().get_display();
    return d == display_inline || d == display_inline_text;
}

// The rows the words `el` lays out itself stand on, and the breaks it
// forces between them: those of its inline children, not of blocks in it.
void lines(const element::ptr& el, std::set<float>& rows, size_t& breaks)
{
    for(auto& child : el->children())
    {
        if(child->is_comment() || child->css().get_display() == display_none) continue;
        if(tag_is(child, "br")) breaks++;
        else if(child->is_text() && !child->is_space() && child->get_render_item())
        {
            position p = child->get_placement();
            if(float(p.height) > 0) rows.insert(float(p.y));
        }
        else if(in_flow(child)) lines(child, rows, breaks);
    }
}

void spread(const element::ptr& el, bool listed)
{
    css_properties& css = el->css();
    listed              = listed || css.get_display() == display_list_item;
    if(css.get_text_align() == text_align_justify || (listed && css.get_text_align() == text_align_center))
        css.set_text_align(text_align_left);
    for(auto& child : el->children()) spread(child, listed);
}
} // namespace

void left_align_spread(const element::ptr& root) { spread(root, false); }

bool left_align_paragraphs(const element::ptr& root)
{
    if(root->is_comment() || root->is_text() || root->css().get_display() == display_none) return false;
    bool changed = false;
    if(root->css().get_text_align() == text_align_center && !in_flow(root))
    {
        std::set<float> rows;
        size_t          breaks = 0;
        lines(root, rows, breaks);
        // Lines the mail broke itself are a list, an address: not wrapped.
        if(rows.size() >= PARAGRAPH_LINES + breaks)
        {
            root->css().set_text_align(text_align_left);
            changed = true;
        }
    }
    for(auto& child : root->children()) changed = left_align_paragraphs(child) || changed;
    return changed;
}
} // namespace cg
