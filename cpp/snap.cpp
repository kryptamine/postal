#include "snap.h"

#include "colour.h"
#include "dom.h"
#include "geometry.h"

using namespace litehtml;

namespace cg
{
namespace
{
bool shown(const css_border& b)
{
    return b.width.val() > 0 && b.style != border_style_none && b.style != border_style_hidden;
}

void hide(css_border& b)
{
    b.style = border_style_none;
    b.width = css_length(0, css_units_px);
}

void snap(css_length& l, float (*to)(float))
{
    if(!l.is_predefined() && l.units() == css_units_px) l.set_value(to(l.val()), css_units_px);
}

bool share(const css_length& l) { return !l.is_predefined() && l.units() == css_units_percentage; }

bool under_a_row(const css_length& l) { return !l.is_predefined() && l.units() == css_units_px && l.val() < CH; }

// A transparent border, or one the colour of the fill under it, takes no
// room. A page-light or grey fill may give way to the terminal's background
// (src/mail_colours.rs), and then the border is the box's shape.
void hide_unseen(css_borders& b, uint32_t fill)
{
    bool fill_may_go = !fill || page_light(fill) || grey(fill);
    for(css_border* e : {&b.top, &b.bottom, &b.left, &b.right})
        if(shown(*e) && (e->color.alpha == 0 || (!fill_may_go && alike(rgb(e->color), fill)))) hide(*e);
}

// Inline-blocks holding blocks are the columns of a fluid layout. On the
// baseline their bottoms line up, which only images of one height hide.
void top_aligned(const element::ptr& el, css_properties& css)
{
    if(css.get_display() != display_inline_block || css.get_vertical_align() != va_baseline) return;
    for(auto& child : el->children())
        if(child->is_block_box() || child->css().get_display() == display_table)
        {
            css.set_vertical_align(va_top);
            return;
        }
}

// A shown border is one cell thick: a row above and below, a column at each
// side. A rule has no content between its edges, so four such borders would
// draw an empty box two rows high: one edge is the line.
void one_cell_thick(css_borders& b, bool rule)
{
    if(shown(b.top)) b.top.width = CH;
    if(shown(b.bottom)) b.bottom.width = CH;
    if(shown(b.left)) b.left.width = CW;
    if(shown(b.right)) b.right.width = CW;
    if(rule && (shown(b.top) || shown(b.bottom)))
    {
        if(shown(b.top)) b.bottom.width = 0;
        b.left.width = b.right.width = 0;
    }
}

// `fill` is the background the nearest ancestor with one is drawn in;
// `listed`, inside a list item.
void snap_box(const element::ptr& el, uint32_t fill, bool listed)
{
    css_properties& css = el->css();
    if(const web_color& own = css.get_bg().m_color; own.alpha) fill = rgb(own);
    top_aligned(el, css);
    // Justified, a line of one-cell glyphs is padded with whole cells; and
    // centred in a list item, the words stand apart from their marker.
    listed = listed || css.get_display() == display_list_item;
    if(css.get_text_align() == text_align_justify || (listed && css.get_text_align() == text_align_center))
        css.set_text_align(text_align_left);

    css_margins m = css.get_margins();
    snap(m.top, rows_above);
    snap(m.bottom, rows_below);
    snap(m.left, columns);
    snap(m.right, columns);
    css.set_margins(m);

    css_borders b = css.get_borders();
    hide_unseen(b, fill);
    // A border is a row of its own, which already sets the text apart: under
    // it, padding short of a full row would only add a blank one.
    css_margins p = css.get_padding();
    if(shown(b.top) && under_a_row(p.top)) p.top.set_value(0, css_units_px);
    if(shown(b.bottom) && under_a_row(p.bottom)) p.bottom.set_value(0, css_units_px);
    snap(p.top, rows_above);
    snap(p.bottom, rows_below);
    snap(p.left, columns);
    snap(p.right, columns);
    css.set_padding(p);
    one_cell_thick(b, tag_is(el, "hr"));
    css.set_borders(b);

    // A share of the page's height has no base: a mail has no page to
    // fill, and `height: 100%` took the 1000-row viewport instead.
    css_length h = css.get_height();
    if(share(h)) h = css_length::predef_value(sizing_auto);
    snap(h, rows_above);
    css.set_height(h);
    css_length min_h = css.get_min_height();
    if(share(min_h)) min_h = css_length(0, css_units_px);
    snap(min_h, rows_above);
    css.set_min_height(min_h);

    css_length spacing = css.get_border_spacing_y();
    snap(spacing, rows_above);
    css.set_border_spacing_y(spacing);
    // A collapsed border gives each cell half its width, which puts a rule
    // between two rows; separate borders with no spacing look the same.
    if(css.get_border_collapse() == border_collapse_collapse)
    {
        css.set_border_collapse(border_collapse_separate);
        css.set_border_spacing_x(css_length(0, css_units_px));
        css.set_border_spacing_y(css_length(0, css_units_px));
    }

    for(auto& child : el->children()) snap_box(child, fill, listed);
}
} // namespace

void snap_tree(const element::ptr& root) { snap_box(root, 0, false); }
} // namespace cg
