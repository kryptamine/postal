#include "label_widths.h"

#include "dom.h"
#include "geometry.h"
#include "text.h"

#include <algorithm>
#include <string>

using namespace litehtml;

namespace cg
{
namespace
{
// An icon may grow to this to show its alt.
const int ICON_CELLS = 16;

// A larger picture, as a badge, may grow to this to show its whole alt.
const int BADGE_CELLS = 32;

// The boxes round `img` that show no word and are narrower than `px` were
// sized for the picture, and would cut its name: they widen to it. True
// when one did.
bool widen(const element::ptr& img, float px)
{
    bool any = false;
    for(auto box = img->parent(); box; box = box->parent())
    {
        std::string text;
        shown_text(box, text);
        if(!squeezed(text).empty()) break;
        float width = px_or(box->css().get_width(), -1);
        if(width < 0 || width >= px) continue;
        box->css().set_width(css_length(px, css_units_px));
        any = true;
    }
    return any;
}

bool past_pane(const element::ptr& el, int cols)
{
    return float(el->get_placement().right()) > float(cols) * CW + 0.5f;
}

// The label spills past a block around it, over its neighbours or under a
// clip, as in a cell of fixed width, or past the pane, widening its table.
bool spills(const element::ptr& el, int cols)
{
    position p = el->get_placement();
    if(past_pane(el, cols)) return true;
    for(auto a = el->parent(); a; a = a->parent())
    {
        if(!a->get_render_item() || a->css().get_display() == display_inline) continue;
        position q = a->get_placement();
        if(p.right() > q.right() + 0.5f || p.left() < q.left() - 0.5f) return true;
    }
    return false;
}
} // namespace

bool LabelWidths::fit(const element::ptr& img, int want, bool icon, bool sole)
{
    css_properties& css = img->css();
    float w = px_or(css.get_width(), -1);
    // Cut to its picture's width an alt names nothing: it shows whole, up
    // to a limit, and an icon's takes a column after it.
    int own = w >= 0 ? int(w / CW) : cols;
    bool grows = w >= 0;
    if(grows && want > own) grown.push_back({img, w});
    int shown = std::min(want, grows ? std::max(own, icon ? ICON_CELLS : BADGE_CELLS) : own);
    if(shown <= 0) return false;

    // A percentage stays: a label any wider would widen its table. A share
    // of a box sized for the picture is the picture's own width, though.
    float px = float(shown + (icon ? 1 : 0)) * CW;
    bool percent = !css.get_width().is_predefined() && css.get_width().units() == css_units_percentage;
    bool cut = percent ? want <= BADGE_CELLS : want > own;
    bool widened = sole && cut && widen(img, px);
    if(!percent || widened) css.set_width(css_length(px, css_units_px));
    css.set_height(css_length(CH, css_units_px));
    css.set_max_width(css_length(100, css_units_percentage));
    return true;
}

bool LabelWidths::let_out_spilt()
{
    bool any = false;
    for(auto& g : grown)
    {
        // Past the pane no box holds it back: the tables before it do.
        if(past_pane(g.el, cols)) continue;
        position p = g.el->get_placement();
        for(auto a = g.el->parent(); a; a = a->parent())
        {
            if(!a->get_render_item() || px_or(a->css().get_width(), -1) < 0) continue;
            if(p.right() <= a->get_placement().right() + 0.5f) continue;
            a->css().set_width(css_length::predef_value(sizing_auto));
            any = true;
        }
    }
    return any;
}

bool LabelWidths::shrink_spilt()
{
    // One row of icons pushes the next off, so a table's grown icons go back
    // to their own width together.
    // Past the pane, the tables before it pushed it there: every one goes.
    std::vector<element::ptr> spilt;
    bool all = false;
    for(auto& g : grown)
    {
        if(spills(g.el, cols)) spilt.push_back(table_of(g.el));
        all = all || past_pane(g.el, cols);
    }
    for(auto& g : grown)
        if(all || std::find(spilt.begin(), spilt.end(), table_of(g.el)) != spilt.end())
            g.el->css().set_width(css_length(columns(g.own_px), css_units_px));
    return !spilt.empty();
}
} // namespace cg
