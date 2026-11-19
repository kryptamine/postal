#include "picture_boxes.h"

#include "dom.h"

#include <cstdlib>
#include <map>
#include <set>
#include <utility>

using namespace litehtml;

namespace cg
{
namespace
{
bool cell(const element::ptr& el) { return tag_is(el, "td") || tag_is(el, "th"); }

bool hidden(const element::ptr& el) { return el->css().get_display() == display_none; }

// The table's own rows, under its sections but not in a table inside it.
std::vector<element::ptr> rows_of(const element::ptr& table)
{
    std::vector<element::ptr> rows;
    for(auto& child : table->children())
        if(tag_is(child, "tr")) rows.push_back(child);
        else if(tag_is(child, "thead") || tag_is(child, "tbody") || tag_is(child, "tfoot"))
            for(auto& row : child->children())
                if(tag_is(row, "tr")) rows.push_back(row);
    return rows;
}

std::vector<element::ptr> cells_of(const element::ptr& row)
{
    std::vector<element::ptr> cells;
    for(auto& child : row->children())
        if(cell(child)) cells.push_back(child);
    return cells;
}

bool spans(const element::ptr& c, const char* attr)
{
    const char* span = c->get_attr(attr);
    return span && std::strtol(span, nullptr, 10) > 1;
}

bool spanned(const element::ptr& table, const char* attr)
{
    for(auto& row : rows_of(table))
        for(auto& c : cells_of(row))
            if(spans(c, attr)) return true;
    return false;
}

bool header(const element::ptr& row)
{
    if(tag_is(row->parent(), "thead")) return true;
    for(auto& c : cells_of(row))
        if(!tag_is(c, "th")) return false;
    return true;
}

void fit(const element::ptr& picture, const Contents& contents)
{
    for(auto box = picture->parent(); box && !hidden(box); box = box->parent())
    {
        if(contents.of(box.get()).sized_by_text()) break;
        box->css().set_height(css_length::predef_value(sizing_auto));
        box->css().set_min_height(css_length(0, css_units_px));
    }
}

// Hides the outermost box round a gone picture that shows nothing else. A
// cell stays, or the columns of its row would shift, and so does a row
// another row spans; returns the outermost empty cell passed, if any.
element::ptr drop_box(const element::ptr& picture, const Contents& contents)
{
    element::ptr empty, emptied_cell;
    for(auto box = picture->parent(); box && !tag_is(box, "body"); box = box->parent())
    {
        if(hidden(box)) return nullptr;
        if(!contents.of(box.get()).shows_nothing()) break;
        if(cell(box)) emptied_cell = box;
        else if(!tag_is(box, "tr") || !spanned(table_of(box), "rowspan")) empty = box;
    }
    if(empty) empty->css().set_display(display_none);
    return emptied_cell;
}

void clear_border(const element::ptr& c, bool right)
{
    css_borders b = c->css().get_borders();
    css_border& e = right ? b.right : b.left;
    e.style = border_style_none;
    e.width = css_length(0, css_units_px);
    c->css().set_borders(b);
}

// Every row has the cell at `column` in the same place only without spans.
// A header names the column, so over a single row it may be the row's
// caption instead, and the column stays.
void drop_column(const element::ptr& table, size_t column, const Contents& contents)
{
    if(spanned(table, "colspan") || spanned(table, "rowspan")) return;
    std::vector<element::ptr> rows;
    int body = 0;
    bool named = false;
    for(auto& row : rows_of(table))
    {
        std::vector<element::ptr> cells = cells_of(row);
        if(hidden(row) || column >= cells.size()) continue;
        rows.push_back(row);
        bool empty = contents.of(cells[column].get()).shows_nothing();
        if(header(row)) named = named || !empty;
        else if(empty) body++;
        else return;
    }
    if(body == 0 || (named && body < 2)) return;

    // A rule between the column and its neighbour ruled off what is gone.
    for(auto& row : rows)
    {
        std::vector<element::ptr> cells = cells_of(row);
        cells[column]->css().set_display(display_none);
        if(column + 1 == cells.size() && column > 0) clear_border(cells[column - 1], true);
        if(column == 0 && cells.size() > 1) clear_border(cells[1], false);
    }
}
} // namespace

void fit_picture_boxes(const std::vector<element::ptr>& pictures, const Contents& contents)
{
    for(auto& picture : pictures) fit(picture, contents);

    std::set<std::pair<element::ptr, size_t>> columns;
    for(auto& picture : pictures)
    {
        if(!hidden(picture)) continue;
        element::ptr c = drop_box(picture, contents);
        if(!c || !tag_is(c->parent(), "tr")) continue;
        std::vector<element::ptr> siblings = cells_of(c->parent());
        for(size_t i = 0; i < siblings.size(); i++)
            if(siblings[i] == c) columns.emplace(table_of(c->parent()), i);
    }
    for(auto& [table, column] : columns)
        if(table) drop_column(table, column, contents);
}
} // namespace cg
