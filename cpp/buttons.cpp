#include "buttons.h"

#include "collapse.h"
#include "colour.h"
#include "dom.h"

using namespace litehtml;

namespace cg
{
namespace
{
// The fill of a button whose border and label are both too light to be one.
const web_color NEUTRAL(128, 128, 128);

// One line of a few words, all a link's, in no filled box further in.
bool label(const element::ptr& el, const Holds& h)
{
    if(h.over || h.words().empty() || h.blocks || h.filled) return false;
    return tag_is(el, "a") || link_of(el) || h.linked;
}

// The innermost filled box round a label is a button; the sections round
// it are not. A page-light fill gives way to the terminal's.
bool filled(const element::ptr& el)
{
    const web_color& bg = el->css().get_bg().m_color;
    return bg.alpha && !page_light(rgb(bg));
}

int cells_in(const element::ptr& el)
{
    int count = el->css().get_display() == display_table_cell;
    for(auto& child : el->children()) count += cells_in(child);
    return count;
}

// A box with a border on every side and no fill of its own round a label.
// A cell of a table with other cells is a cell of data that holds a link.
bool outlined(const element::ptr& el)
{
    const css_borders& b = el->css().get_borders();
    for(const css_border* e : {&b.top, &b.bottom, &b.left, &b.right})
        if(e->width.val() <= 0 || e->style == border_style_none || e->style == border_style_hidden ||
           e->color.alpha == 0)
            return false;
    if(el->css().get_display() != display_table_cell) return true;
    auto table = table_of(el);
    return table && cells_in(table) == 1;
}

// The border's colour, or the label's where the border is too light to
// stand as a fill on a page.
void fill_from_border(const element::ptr& el)
{
    css_properties& css = el->css();
    web_color       fill = css.get_borders().top.color;
    if(page_light(rgb(fill))) fill = css.get_color();
    if(page_light(rgb(fill))) fill = NEUTRAL;
    background bg = css.get_bg();
    bg.m_color    = fill;
    css.set_bg(bg);
}

void flatten(const element::ptr& el, const Contents& contents, std::vector<element::ptr>& out)
{
    // A word has no box of its own, and litehtml leaves its styles unset.
    if(el->is_text() || el->is_comment()) return;
    if(label(el, contents.of(el.get())) && (filled(el) || outlined(el)))
    {
        if(!filled(el)) fill_from_border(el);
        // A slab three rows high reads as no button, and a border a cell
        // thick only adds a row to the fill that is its shape.
        collapse(el);
        out.push_back(el);
        return;
    }
    for(auto& child : el->children()) flatten(child, contents, out);
}
} // namespace

std::vector<element::ptr> flatten_buttons(const element::ptr& root, const Contents& contents)
{
    std::vector<element::ptr> buttons;
    flatten(root, contents, buttons);
    return buttons;
}
} // namespace cg
