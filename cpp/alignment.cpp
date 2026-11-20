#include "alignment.h"

#include "dom.h"
#include "text.h"

#include <cctype>

#include <set>
#include <string>
#include <vector>

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

// A table row of two cells whose first shows one glyph that is no letter
// or digit, a bullet, is a list item built of cells.
bool bulleted(const element::ptr& row)
{
    if(!tag_is(row, "tr")) return false;
    std::vector<element::ptr> cells_in;
    for(auto& cell : row->children())
        if(cell->css().get_display() == display_table_cell) cells_in.push_back(cell);
    if(cells_in.size() != 2) return false;
    std::string text;
    shown_text(cells_in[0], text);
    text = squeezed(text);
    return cells(text.c_str()) == 1 && !std::isalnum(static_cast<unsigned char>(text[0]));
}

void spread(const element::ptr& el, bool listed)
{
    css_properties& css = el->css();
    listed = listed || css.get_display() == display_list_item || bulleted(el);
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
        size_t breaks = 0;
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
