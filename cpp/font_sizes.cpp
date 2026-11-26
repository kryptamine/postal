#include "font_sizes.h"

#include "hidden.h"
#include "text.h"

#include <algorithm>
#include <map>
#include <string>
#include <vector>

using namespace litehtml;

namespace cg
{
namespace
{
const float BODY_PX = 13;
// This share of the body's size or smaller is the small print.
const float SMALL_PRINT = 0.85f;
// This share of the body's size and this many px or larger is a heading:
// an h3 at 18 px over a 16 px body is not.
const float TITLE = 1.4f;
const float TITLE_PX = 18;
// A mail with fewer rows of the body's words has no body to set the small
// print off from.
const int BODY_ROWS = 3;

void letters(const element::ptr& el, std::map<float, size_t>& count)
{
    if(unseen(el)) return;
    if(el->is_text())
    {
        std::string text;
        el->get_text(text);
        if(size_t n = squeezed(text).size()) count[float(el->css().get_font_metrics().font_size)] += n;
        return;
    }
    for(auto& child : el->children()) letters(child, count);
}

bool worded(const cg_cell& c) { return (c.flags & CG_TEXT) && !(c.flags & (CG_ALT | CG_BORDER)) && !is(c, " "); }

bool body_word(const cg_cell& c) { return worded(c) && !(c.flags & CG_SMALL) && c.link == 0; }
} // namespace

float body_size(const element::ptr& root)
{
    std::map<float, size_t> count;
    letters(root, count);
    auto most = [](auto& a, auto& b) { return a.second < b.second; };
    auto from = count.lower_bound(BODY_PX) == count.end() ? count.begin() : count.lower_bound(BODY_PX);
    return from == count.end() ? 0 : std::max_element(from, count.end(), most)->first;
}

uint32_t size_flags(float size, float body)
{
    if(body <= 0) return 0;
    if(size <= body * SMALL_PRINT) return CG_SMALL;
    return size >= std::max(body * TITLE, TITLE_PX) ? CG_TITLE : 0;
}

void keep_small_print(Grid& grid)
{
    int rows = grid.rows();
    auto row_has = [&grid](int r, bool (*test)(const cg_cell&)) {
        auto row = grid.drawn.begin() + std::ptrdiff_t(r) * grid.cols;
        return std::any_of(row, row + grid.cols, test);
    };
    std::vector<int> body;
    for(int r = 0; r < rows; r++)
        if(row_has(r, body_word)) body.push_back(r);
    int start = rows;
    if(int(body.size()) >= BODY_ROWS)
        for(int r = body.back() + 1; r < rows && start == rows; r++)
            if(!row_has(r - 1, worded)) start = r;
    for(int i = 0; i < start * grid.cols; i++) grid.drawn[size_t(i)].flags &= ~uint32_t(CG_SMALL);
}
} // namespace cg
