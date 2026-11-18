#include "image_labels.h"

#include "geometry.h"
#include "text.h"

#include <algorithm>
#include <cstring>

using namespace litehtml;

namespace cg
{
namespace
{
bool tag_is(const element::ptr& el, const char* tag) { return std::strcmp(el->get_tagName(), tag) == 0; }

float px_or(const css_length& l, float none)
{
    return !l.is_predefined() && l.units() == css_units_px ? float(l.val()) : none;
}

// The text a reader sees: litehtml's `get_text` takes in comments too, and
// mail hides whole Outlook tables in conditional ones.
void shown_text(const element::ptr& el, std::string& out)
{
    if(el->is_comment() || el->css().get_display() == display_none) return;
    if(el->is_text() || el->is_space()) el->get_text(out);
    for(auto& child : el->children()) shown_text(child, out);
}

element::ptr link_of(const element::ptr& el)
{
    for(auto p = el->parent(); p; p = p->parent())
        if(tag_is(p, "a"))
            if(const char* href = p->get_attr("href"); href && *href) return p;
    return nullptr;
}

element::ptr table_of(const element::ptr& el)
{
    auto p = el->parent();
    while(p && !tag_is(p, "table")) p = p->parent();
    return p;
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

// The label spills past a block around it, over its neighbours or under a
// clip, as in a cell of fixed width, or past the pane, widening its table.
bool spills(const element::ptr& el, int cols)
{
    position p = el->get_placement();
    if(float(p.right()) > float(cols) * CW + 0.5f) return true;
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
    for(auto& child : root->children()) label(child, width);
    if(tag_is(root, "img") && root->css().get_display() != display_none) label_one(root);
}

// What an image shows, before any cut to its box; empty when it goes.
std::string ImageLabels::text_for(const element::ptr& el, bool icon, const element::ptr& link, bool sole)
{
    const css_properties& css = el->css();
    float       w = px_or(css.get_width(), -1), h = px_or(css.get_height(), -1);
    const char* alt   = el->get_attr("alt");
    std::string label = squeezed(alt ? alt : "");
    bool invisible    = (w >= 0 && w < CW) || (h >= 0 && h <= 2);
    // An icon decorates the text beside it, and its alt rarely fits it.
    if(invisible || (icon && !sole) || repeats_nearby(el, label)) return "";
    if(!label.empty() || !sole || icon) return label;
    if(std::find(labelled_links.begin(), labelled_links.end(), link.get()) != labelled_links.end()) return "";
    labelled_links.push_back(link.get());
    return "[link]";
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
    int want = cells(label.c_str());
    int own  = w >= 0 ? int(w / CW) : cols;
    if(icon && want > own && w >= 0) grown.push_back({el, w});
    int shown = std::min(want, icon && w >= 0 ? std::max(own, 16) : own);
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
    std::vector<element::ptr> spilt;
    for(auto& g : grown)
        if(spills(g.el, cols)) spilt.push_back(table_of(g.el));
    for(auto& g : grown)
        if(std::find(spilt.begin(), spilt.end(), table_of(g.el)) != spilt.end())
            g.el->css().set_width(css_length(columns(g.own_px), css_units_px));
    return !spilt.empty();
}

const std::string* ImageLabels::of(const std::string& src) const
{
    auto it = labels.find(src);
    return it == labels.end() ? nullptr : &it->second;
}
} // namespace cg
