#include "buttons.h"

#include "collapse.h"
#include "colour.h"
#include "dom.h"

#include <algorithm>
#include <cstdint>

using namespace litehtml;

namespace cg
{
namespace
{
// The cells of a button's fill at each side of its label.
const int BUTTON_PAD = 2;

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
    web_color fill = css.get_borders().top.color;
    if(page_light(rgb(fill))) fill = css.get_color();
    // Both too light: mid grey.
    if(page_light(rgb(fill))) fill = web_color(128, 128, 128);
    background bg = css.get_bg();
    bg.m_color = fill;
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

// The cell shows `fill` and nothing else, and no row over or under the
// button carries the fill on: that would be a section it lies in.
bool fill_only(Grid& grid, const Box& button, int r, int c, uint32_t fill)
{
    auto filled = [&](int row) {
        return row >= 0 && row < grid.rows() && (grid.at(row, c)->bg & ~CG_BG_PANEL) == fill;
    };
    return fill && filled(r) && grid.at(r, c)->flags == 0 && !filled(button.r0 - 1) && !filled(button.r1 + 1);
}

// The cells the words of `button` stand in: an inline one has no box of
// its own to ask for. Empty when it shows none.
Box words_of(const element::ptr& button, Grid& grid)
{
    std::vector<position> words;
    leaf_boxes(button, words);
    Box box{INT32_MAX, -1, INT32_MAX, -1};
    for(auto& p : words)
    {
        Box word = cells_of(p);
        box = {std::min(box.r0, word.r0), std::max(box.r1, word.r1), std::min(box.c0, word.c0),
               std::max(box.c1, word.c1)};
    }
    return {std::max(box.r0, 0), std::min(box.r1, grid.rows() - 1), std::max(box.c0, 0),
            std::min(box.c1, grid.cols - 1)};
}

// `box` holds the button's words; the fill round them is cut back or let
// out to BUTTON_PAD cells past the first and the last glyph of any row.
void shape(Grid& grid, const Box& box)
{
    int first = INT32_MAX, last = -1;
    for(int r = box.r0; r <= box.r1; r++)
        for(int c = box.c0; c <= box.c1; c++)
            if(cg_cell& cell = *grid.at(r, c); (cell.flags & CG_TEXT) && !is(cell, " "))
            {
                first = std::min(first, c);
                last = std::max(last, c);
            }
    if(last < 0) return;
    int from = first - BUTTON_PAD, to = last + BUTTON_PAD, end = grid.cols - 1;
    for(int r = box.r0; r <= box.r1; r++)
    {
        uint32_t fill = 0;
        for(int c = box.c0; c <= box.c1 && !fill; c++)
            if(grid.at(r, c)->flags & CG_TEXT) fill = grid.at(r, c)->bg & ~CG_BG_PANEL;
        // A box round the button in its own fill is part of its shape.
        int c0 = box.c0, c1 = box.c1;
        while(c0 > 0 && fill_only(grid, box, r, c0 - 1, fill)) c0--;
        while(c1 < end && fill_only(grid, box, r, c1 + 1, fill)) c1++;
        uint32_t left = c0 > 0 ? grid.at(r, c0 - 1)->bg : 0;
        uint32_t right = c1 < end ? grid.at(r, c1 + 1)->bg : 0;
        for(int c = std::max(std::min(c0, from), 0); c <= std::min(std::max(c1, to), end); c++)
        {
            cg_cell& cell = *grid.at(r, c);
            if(c < from) cell.bg = left;
            else if(c > to) cell.bg = right;
            // The fill takes its box, and outside it only cells that show nothing.
            else if((c >= c0 && c <= c1) || (cell.flags == 0 && is(cell, " "))) cell.bg = fill;
            // The fill says it can be pressed; a link's underline is noise.
            if(cell.flags & CG_TEXT) cell.flags = (cell.flags | CG_BOLD) & ~uint32_t(CG_UNDERLINE);
        }
    }
}
} // namespace

void shape_buttons(const std::vector<element::ptr>& buttons, Grid& grid)
{
    for(auto& button : buttons) shape(grid, words_of(button, grid));
}

std::vector<element::ptr> flatten_buttons(const element::ptr& root, const Contents& contents)
{
    std::vector<element::ptr> buttons;
    flatten(root, contents, buttons);
    return buttons;
}
} // namespace cg
