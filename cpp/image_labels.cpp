#include "image_labels.h"

#include "alt_names.h"
#include "dom.h"
#include "geometry.h"
#include "link_names.h"
#include "text.h"

#include <algorithm>
#include <set>
#include <utility>

using namespace litehtml;

namespace cg
{
namespace
{
// An alt this long is a title rather than a name.
const int TITLE_CELLS = 16;

// A link's own picture, a badge, may grow to this to show its whole alt.
const int BADGE_CELLS = 32;

float px_or(const css_length& l, float none)
{
    return !l.is_predefined() && l.units() == css_units_px ? float(l.val()) : none;
}

// Mail tags each link with where in the mail it sits, in the query.
std::string page_of(const char* href)
{
    std::string url(href);
    return url.substr(0, url.find_first_of("?#"));
}

// A picture says something or opens something; a spacer image does neither,
// and its box keeps the gap it was put there for.
bool picture(const element::ptr& img)
{
    const char* alt = img->get_attr("alt");
    if(alt && *alt) return true;
    for(auto p = img->parent(); p; p = p->parent())
        if(tag_is(p, "a")) return true;
    return false;
}

struct LinkScan
{
    // The pages links with words open, and each link with none.
    std::set<std::string>                               worded;
    std::vector<std::pair<const element*, std::string>> bare;
    // Bare links since the last words, and those the next words were a link's.
    std::vector<const element*> pending;
    std::set<const element*>    followed;
};

void scan(const element::ptr& el, LinkScan& s)
{
    if(el->is_comment() || el->css().get_display() == display_none) return;
    if(el->is_text())
    {
        std::string text;
        el->get_text(text);
        if(squeezed(text).empty()) return;
        if(link_of(el)) s.followed.insert(s.pending.begin(), s.pending.end());
        s.pending.clear();
        return;
    }
    if(tag_is(el, "a"))
        if(const char* href = el->get_attr("href"); href && *href)
        {
            std::string text;
            shown_text(el, text);
            if(!squeezed(text).empty()) s.worded.insert(page_of(href));
            else
            {
                s.pending.push_back(el.get());
                s.bare.emplace_back(el.get(), page_of(href));
            }
        }
    for(auto& child : el->children()) scan(child, s);
}

// Links that show nothing but whose page a button or a title opens too: a
// product's picture beside its title, or a banner over its button.
std::set<const element*> redundant_links(const element::ptr& root)
{
    LinkScan s;
    scan(root, s);
    for(auto& [link, page] : s.bare)
        if(s.worded.count(page)) s.followed.insert(link);
    return s.followed;
}

// The nearest text around `el` already says `alt`, as a caption or a
// name beside an avatar or a status icon does.
bool repeats_nearby(const element::ptr& el, const std::string& alt)
{
    if(alt.empty()) return false;
    auto p = el->parent();
    for(int depth = 0; p && depth < 5; depth++, p = p->parent())
    {
        // A table's other rows are other content, not this image's row.
        if(tag_is(p, "table") || tag_is(p, "tbody") || tag_is(p, "thead")) return false;
        std::string text;
        shown_text(p, text);
        text = squeezed(text);
        // Past a caption's or a table row's length, the text is the mail's.
        if(!text.empty()) return text.size() <= 200 && text.find(alt) != std::string::npos;
    }
    return false;
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

void ImageLabels::label(const element::ptr& root, int width)
{
    cols = width;
    redundant = redundant_links(root);
    shown_text(root, mail_text);
    mail_text = squeezed(mail_text);
    walk(root);
}

void ImageLabels::walk(const element::ptr& el)
{
    for(auto& child : el->children()) walk(child);
    if(!tag_is(el, "img")) return;
    if(picture(el)) pictured.push_back(el);
    if(el->css().get_display() != display_none) label_one(el);
}

// What an image shows, before any cut to its box; empty when it goes.
std::string ImageLabels::text_for(const element::ptr& el, bool icon, const element::ptr& link, bool sole)
{
    const css_properties& css = el->css();
    float       w = px_or(css.get_width(), -1), h = px_or(css.get_height(), -1);
    const char* alt   = el->get_attr("alt");
    std::string label = alt_name(squeezed(alt ? alt : ""));
    bool invisible    = (w >= 0 && w < CW) || (h >= 0 && h <= 2);
    // An icon decorates the text beside it, and its alt rarely fits it.
    if(invisible || (icon && !sole) || repeats_nearby(el, label)) return "";
    // A title the mail shows as text already is a card's picture beside its
    // caption, however far the markup puts it; a name is looked for nearby.
    if(cells(label.c_str()) >= TITLE_CELLS && mail_text.find(label) != std::string::npos) return "";
    if(!label.empty() || !sole || icon) return label;
    if(labelled_links.count(link.get()) || redundant.count(link.get())) return "";
    labelled_links.insert(link.get());
    return "[" + link_name(link->get_attr("href")) + "]";
}

void ImageLabels::label_one(const element::ptr& el)
{
    css_properties& css  = el->css();
    float           w    = px_or(css.get_width(), -1), h = px_or(css.get_height(), -1);
    bool            icon = (w >= 0 || h >= 0) && w <= 64 && h <= 64;
    element::ptr    link = link_of(el);
    std::string     link_text;
    if(link) shown_text(link, link_text);
    bool        sole  = link && squeezed(link_text).empty();
    std::string label = text_for(el, icon, link, sole);

    // An icon left is all its link shows, so its alt names the link, and
    // cut it names nothing: it takes its whole alt, and a column after it.
    // So does a badge, a larger picture that is all its link shows.
    int  want  = cells(label.c_str());
    int  own   = w >= 0 ? int(w / CW) : cols;
    bool grows = w >= 0 && (icon || sole);
    if(grows && want > own) grown.push_back({el, w});
    int shown = std::min(want, grows ? std::max(own, icon ? TITLE_CELLS : BADGE_CELLS) : own);
    if(shown <= 0)
    {
        css.set_display(display_none);
        return;
    }

    // A percentage stays: a label any wider would widen its table.
    bool percent = !css.get_width().is_predefined() && css.get_width().units() == css_units_percentage;
    if(!percent) css.set_width(css_length(float(shown + (icon ? 1 : 0)) * CW, css_units_px));
    css.set_height(css_length(CH, css_units_px));
    css.set_max_width(css_length(100, css_units_percentage));
    // Each image gets a src of its own: one icon file carries many alts.
    std::string key = "postal:image/" + std::to_string(labels.size());
    el->set_attr("src", key.c_str());
    el->parse_attributes();
    labels.emplace(key, label);
}

bool ImageLabels::shrink_spilt()
{
    // One row of icons pushes the next off, so a table's grown icons go back
    // to their own width together.
    // Past the pane, the tables before it pushed it there: every one goes.
    std::vector<element::ptr> spilt;
    bool                      all = false;
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

const std::string* ImageLabels::of(const std::string& src) const
{
    auto it = labels.find(src);
    return it == labels.end() ? nullptr : &it->second;
}
} // namespace cg
