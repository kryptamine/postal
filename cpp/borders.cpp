#include "borders.h"

#include "colour.h"
#include "dom.h"

#include <algorithm>
#include <array>
#include <cstdint>
#include <cstring>
#include <utility>

using namespace litehtml;

namespace cg
{
namespace
{
enum Dir : uint8_t
{
    DIR_UP = 1,
    DIR_DOWN = 2,
    DIR_LEFT = 4,
    DIR_RIGHT = 8,
};

enum class Stroke : uint8_t
{
    solid,
    dashed,
    double_line,
};

struct Edge
{
    uint8_t dirs = 0;
    bool round = false;
    Stroke stroke = Stroke::solid;
};

// One side of a frame: the cells `from` to `to` along row (or column) `at`.
// At an end another side meets, the line turns into a corner.
struct Side
{
    const border& b;
    bool across;
    int at, from, to;
    bool met_from, met_to;
    bool round_from, round_to;
};

Stroke stroke_of(border_style s)
{
    if(s == border_style_dashed || s == border_style_dotted) return Stroke::dashed;
    if(s == border_style_double) return Stroke::double_line;
    return Stroke::solid;
}

bool inside_clips(int row, int col, const std::vector<position>& clips)
{
    return std::all_of(clips.begin(), clips.end(), [row, col](const position& p) { return centre_in(row, col, p); });
}

bool row_has_text(const Grid& grid, int row, int c0, int c1)
{
    if(row < 0 || row >= grid.rows()) return false;
    for(int c = std::max(c0, 0); c <= std::min(c1, grid.cols - 1); c++)
    {
        const cg_cell& cell = grid.drawn[size_t(row) * size_t(grid.cols) + size_t(c)];
        if((cell.flags & CG_TEXT) && !is(cell, " ")) return true;
    }

    return false;
}

// A horizontal edge on a row of text moves one row out, to `out`, or drops
// when that row has text too.
void step_off_text(const Grid& grid, bool& on, int& row, int out, int c0, int c1, bool left, bool right)
{
    if(!on || !row_has_text(grid, row, c0 + left, c1 - right)) return;
    if(row_has_text(grid, out, c0, c1)) on = false;
    else row = out;
}

void mark(Grid& grid, std::vector<Edge>& edges, int row, int col, uint8_t dirs, bool round, const border& b,
          uint32_t flags, const std::vector<position>& clips)
{
    cg_cell* c = grid.at(row, col);
    // `at` grows the grid down to a border stepped below the last row.
    if(edges.size() < grid.drawn.size()) edges.resize(grid.drawn.size());
    if(!c || (c->flags & CG_TEXT) || !inside_clips(row, col, clips)) return;
    Edge& edge = edges[size_t(row) * size_t(grid.cols) + size_t(col)];
    edge.dirs |= dirs;
    edge.round = edge.round || round;
    if(edge.stroke == Stroke::solid) edge.stroke = stroke_of(b.style);
    c->fg = over(c->bg, b.color);
    // An inner border overlapping the page box keeps the inner one visible.
    c->flags = (c->flags & CG_BORDER) ? (c->flags & flags) : flags;
}

void draw_side(Grid& grid, std::vector<Edge>& edges, const Side& s, uint32_t flags, const std::vector<position>& clips)
{
    uint8_t back = s.across ? DIR_LEFT : DIR_UP, ahead = s.across ? DIR_RIGHT : DIR_DOWN;
    for(int i = s.from; i <= s.to; i++)
    {
        auto dirs = uint8_t((i > s.from || !s.met_from ? back : 0) | (i < s.to || !s.met_to ? ahead : 0));
        bool round = (i == s.from && s.met_from && s.round_from) || (i == s.to && s.met_to && s.round_to);
        mark(grid, edges, s.across ? s.at : i, s.across ? i : s.at, dirs, round, s.b, flags, clips);
    }
}

bool joins(const cg_cell& a, const cg_cell& b)
{
    if(!(a.flags & CG_BORDER) || !(b.flags & CG_BORDER)) return false;
    if((a.flags & CG_PAGE_BOX) || (b.flags & CG_PAGE_BOX)) return false;
    return alike(a.fg, b.fg) || (grey(a.fg) && grey(b.fg));
}

// Connects borders whose lines touch in adjacent cells into T-junctions.
void join_touching(const Grid& grid, std::vector<Edge>& edges)
{
    const std::vector<Edge> orig = edges;
    int rows = grid.rows(), cols = grid.cols;
    auto stride = size_t(cols);
    for(int r = 0; r < rows; r++)
        for(int c = 0; c < cols; c++)
        {
            size_t i = size_t(r) * stride + size_t(c);
            uint8_t d = orig[i].dirs;
            if(!d || (grid.drawn[i].flags & CG_PAGE_BOX)) continue;
            auto link = [&](bool there, size_t j, uint8_t facing, uint8_t dir) {
                if(there && (orig[j].dirs & facing) && joins(grid.drawn[i], grid.drawn[j])) edges[i].dirs |= dir;
            };
            if(d & (DIR_UP | DIR_DOWN))
            {
                link(c + 1 < cols, i + 1, DIR_LEFT, DIR_RIGHT);
                link(c > 0, i - 1, DIR_RIGHT, DIR_LEFT);
            }

            if(d & (DIR_LEFT | DIR_RIGHT))
            {
                link(r + 1 < rows, i + stride, DIR_UP, DIR_DOWN);
                link(r > 0, i - stride, DIR_DOWN, DIR_UP);
            }
        }
}

const char* glyph_for(const Edge& e)
{
    switch(e.dirs)
    {
    case DIR_LEFT | DIR_RIGHT:
    case DIR_LEFT:
    case DIR_RIGHT:
        return e.stroke == Stroke::dashed ? "┄" : e.stroke == Stroke::double_line ? "═" : "─";
    case DIR_UP | DIR_DOWN:
    case DIR_UP:
    case DIR_DOWN:
        return e.stroke == Stroke::dashed ? "┆" : e.stroke == Stroke::double_line ? "║" : "│";
    case DIR_DOWN | DIR_RIGHT:
        return e.round ? "╭" : "┌";
    case DIR_DOWN | DIR_LEFT:
        return e.round ? "╮" : "┐";
    case DIR_UP | DIR_RIGHT:
        return e.round ? "╰" : "└";
    case DIR_UP | DIR_LEFT:
        return e.round ? "╯" : "┘";
    case DIR_UP | DIR_DOWN | DIR_RIGHT:
        return "├";
    case DIR_UP | DIR_DOWN | DIR_LEFT:
        return "┤";
    case DIR_DOWN | DIR_LEFT | DIR_RIGHT:
        return "┬";
    case DIR_UP | DIR_LEFT | DIR_RIGHT:
        return "┴";
    case DIR_UP | DIR_DOWN | DIR_LEFT | DIR_RIGHT:
        return "┼";
    default:
        return nullptr;
    }
}
} // namespace

bool apply_borders(Grid& grid, const std::vector<Frame>& frames, const Box& mail)
{
    bool lost = false;
    std::vector<Edge> edges(grid.drawn.size());
    for(const auto& f : frames)
    {
        auto [r0, r1, c0, c1] = f.box;
        bool top = shown(f.b.top), bottom = shown(f.b.bottom), left = shown(f.b.left), right = shown(f.b.right);
        lost = lost || (left && c0 < 0) || (right && c1 >= grid.cols);
        bool page = top && bottom && left && right && f.box.holds(mail);
        uint32_t flags = page ? CG_BORDER | CG_PAGE_BOX : CG_BORDER;
        int tr = r0, br = r1;
        step_off_text(grid, top, tr, r0 - 1, c0, c1, left, right);
        step_off_text(grid, bottom, br, r1 + 1, c0, c1, left, right);
        const border_radiuses& rad = f.b.radius;
        bool tl = rounded(rad.top_left_x, rad.top_left_y), tright = rounded(rad.top_right_x, rad.top_right_y);
        bool bl = rounded(rad.bottom_left_x, rad.bottom_left_y);
        bool bright = rounded(rad.bottom_right_x, rad.bottom_right_y);
        const std::array<std::pair<bool, Side>, 4> sides{{
            {top, {f.b.top, true, tr, c0, c1, left, right, tl, tright}},
            {bottom, {f.b.bottom, true, br, c0, c1, left, right, bl, bright}},
            {left, {f.b.left, false, c0, tr, br, top, bottom, tl, bl}},
            {right, {f.b.right, false, c1, tr, br, top, bottom, tright, bright}},
        }};
        for(const auto& [on, side] : sides)
            if(on) draw_side(grid, edges, side, flags, f.clips);
    }

    join_touching(grid, edges);
    for(size_t i = 0; i < edges.size(); i++)
        if(const char* g = glyph_for(edges[i])) set_glyph(grid.drawn[i], g, std::strlen(g));
    return lost;
}
} // namespace cg
