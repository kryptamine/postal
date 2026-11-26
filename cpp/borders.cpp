#include "borders.h"

#include "colour.h"
#include "geometry.h"

#include <algorithm>
#include <cstdint>
#include <cstring>

using namespace litehtml;

namespace cg
{
namespace
{
// A corner rounded this much or more is drawn round, as `╭`.
const float ROUND_PX = 4;

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

bool is_round(pixel_t x, pixel_t y) { return float(x) >= ROUND_PX && float(y) >= ROUND_PX; }

bool shown(const border& e)
{
    return float(e.width) > 0.f && e.style != border_style_none && e.style != border_style_hidden;
}

Stroke stroke_of(border_style s)
{
    if(s == border_style_dashed || s == border_style_dotted) return Stroke::dashed;
    if(s == border_style_double) return Stroke::double_line;
    return Stroke::solid;
}

bool inside_clips(int row, int col, const std::vector<position>& clips)
{
    float x = (float(col) + 0.5f) * CW, y = (float(row) + 0.5f) * CH;
    return std::all_of(clips.begin(), clips.end(), [x, y](const position& p) {
        float px = p.x, py = p.y, pw = p.width, ph = p.height;
        return x >= px && x < px + pw && y >= py && y < py + ph;
    });
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

void set_glyph(cg_cell& c, const char* g)
{
    size_t n = std::min(std::strlen(g), sizeof c.glyph - 1);
    std::memcpy(c.glyph, g, n);
    c.glyph[n] = 0;
}

void mark(Grid& grid, std::vector<Edge>& edges, int row, int col, uint8_t dirs, const border& b, uint32_t flags,
          const std::vector<position>& clips, bool round = false)
{
    cg_cell* c = grid.at(row, col);
    if(!c || (c->flags & CG_TEXT) || !inside_clips(row, col, clips)) return;
    if(edges.size() < grid.drawn.size()) edges.resize(grid.drawn.size());
    size_t idx = size_t(row) * size_t(grid.cols) + size_t(col);
    Edge& edge = edges[idx];
    edge.dirs |= dirs;
    if(round) edge.round = true;
    if(edge.stroke == Stroke::solid) edge.stroke = stroke_of(b.style);
    c->fg = b.color.alpha == 255 ? rgb(b.color) : blend(c->bg, rgb(b.color), float(b.color.alpha) / 255.f);
    // An inner border overlapping the page box keeps the inner one visible.
    c->flags = (c->flags & CG_BORDER) ? (c->flags & flags) : flags;
}

bool joins(const cg_cell& a, const cg_cell& b)
{
    if(!(a.flags & CG_BORDER) || !(b.flags & CG_BORDER)) return false;
    if((a.flags & CG_PAGE_BOX) || (b.flags & CG_PAGE_BOX)) return false;
    return alike(a.fg, b.fg) || (grey(a.fg) && grey(b.fg));
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

void apply_borders(Grid& grid, const std::vector<Frame>& frames, const Box& mail, bool& lost_border)
{
    std::vector<Edge> edges(grid.drawn.size());
    for(const auto& f : frames)
    {
        auto [r0, r1, c0, c1] = f.box;
        bool top = shown(f.b.top), bottom = shown(f.b.bottom), left = shown(f.b.left), right = shown(f.b.right);
        lost_border = lost_border || (left && c0 < 0) || (right && c1 >= grid.cols);
        bool page = top && bottom && left && right && f.box.holds(mail);
        uint32_t flags = page ? CG_BORDER | CG_PAGE_BOX : CG_BORDER;
        int tr = r0, br = r1;
        // A horizontal edge on a row of text moves one row out, or drops.
        if(top && row_has_text(grid, tr, c0 + left, c1 - right))
        {
            tr = r0 - 1;
            if(row_has_text(grid, tr, c0, c1))
            {
                top = false;
                tr = r0;
            }
        }

        if(bottom && row_has_text(grid, br, c0 + left, c1 - right))
        {
            br = r1 + 1;
            if(row_has_text(grid, br, c0, c1))
            {
                bottom = false;
                br = r1;
            }
        }

        const border_radiuses& rad = f.b.radius;
        if(top)
            for(int c = c0; c <= c1; c++)
            {
                auto d = uint8_t((c > c0 || !left ? DIR_LEFT : 0) | (c < c1 || !right ? DIR_RIGHT : 0));
                bool r = (c == c0 && left && is_round(rad.top_left_x, rad.top_left_y)) ||
                         (c == c1 && right && is_round(rad.top_right_x, rad.top_right_y));
                mark(grid, edges, tr, c, d, f.b.top, flags, f.clips, r);
            }

        if(bottom)
            for(int c = c0; c <= c1; c++)
            {
                auto d = uint8_t((c > c0 || !left ? DIR_LEFT : 0) | (c < c1 || !right ? DIR_RIGHT : 0));
                bool r = (c == c0 && left && is_round(rad.bottom_left_x, rad.bottom_left_y)) ||
                         (c == c1 && right && is_round(rad.bottom_right_x, rad.bottom_right_y));
                mark(grid, edges, br, c, d, f.b.bottom, flags, f.clips, r);
            }

        if(left)
            for(int r = tr; r <= br; r++)
            {
                auto d = uint8_t((r > tr || !top ? DIR_UP : 0) | (r < br || !bottom ? DIR_DOWN : 0));
                bool r_corner = (r == tr && top && is_round(rad.top_left_x, rad.top_left_y)) ||
                                (r == br && bottom && is_round(rad.bottom_left_x, rad.bottom_left_y));
                mark(grid, edges, r, c0, d, f.b.left, flags, f.clips, r_corner);
            }

        if(right)
            for(int r = tr; r <= br; r++)
            {
                auto d = uint8_t((r > tr || !top ? DIR_UP : 0) | (r < br || !bottom ? DIR_DOWN : 0));
                bool r_corner = (r == tr && top && is_round(rad.top_right_x, rad.top_right_y)) ||
                                (r == br && bottom && is_round(rad.bottom_right_x, rad.bottom_right_y));
                mark(grid, edges, r, c1, d, f.b.right, flags, f.clips, r_corner);
            }
    }

    // Connect borders whose lines touch in adjacent cells into T-junctions.
    std::vector<Edge> orig = edges;
    int rows = grid.rows(), cols = grid.cols;
    for(int r = 0; r < rows; r++)
        for(int c = 0; c < cols; c++)
        {
            size_t i = size_t(r) * size_t(cols) + size_t(c);
            uint8_t d = orig[i].dirs;
            if(!d || (grid.drawn[i].flags & CG_PAGE_BOX)) continue;
            if(d & (DIR_UP | DIR_DOWN))
            {
                if(c + 1 < cols && (orig[i + 1].dirs & DIR_LEFT) && joins(grid.drawn[i], grid.drawn[i + 1]))
                    edges[i].dirs |= DIR_RIGHT;
                if(c > 0 && (orig[i - 1].dirs & DIR_RIGHT) && joins(grid.drawn[i], grid.drawn[i - 1]))
                    edges[i].dirs |= DIR_LEFT;
            }

            if(d & (DIR_LEFT | DIR_RIGHT))
            {
                if(r + 1 < rows && (orig[i + size_t(cols)].dirs & DIR_UP) &&
                   joins(grid.drawn[i], grid.drawn[i + size_t(cols)]))
                    edges[i].dirs |= DIR_DOWN;
                if(r > 0 && (orig[i - size_t(cols)].dirs & DIR_DOWN) &&
                   joins(grid.drawn[i], grid.drawn[i - size_t(cols)]))
                    edges[i].dirs |= DIR_UP;
            }
        }

    for(size_t i = 0; i < edges.size(); i++)
        if(const char* g = glyph_for(edges[i])) set_glyph(grid.drawn[i], g);
}
} // namespace cg
