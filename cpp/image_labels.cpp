#include "image_labels.h"

#include "alt_names.h"
#include "dom.h"
#include "geometry.h"
#include "link_names.h"
#include "said_already.h"
#include "shared_prefix.h"
#include "text.h"

#include <set>
#include <utility>

using namespace litehtml;

namespace cg
{
namespace
{
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
    std::set<std::string> worded;
    std::vector<std::pair<const element*, std::string>> bare;
    // Bare links since the last words, and those the next words were a link's.
    std::vector<const element*> pending;
    std::set<const element*> followed;
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

bool sized_icon(const element::ptr& img)
{
    float w = px_or(img->css().get_width(), -1), h = px_or(img->css().get_height(), -1);
    return (w >= 0 || h >= 0) && w <= 64 && h <= 64;
}

// Under one column wide or 2 px high: a tracking pixel or a spacer.
bool invisible(const element::ptr& img)
{
    float w = px_or(img->css().get_width(), -1), h = px_or(img->css().get_height(), -1);
    return (w >= 0 && w < CW) || (h >= 0 && h <= 2);
}

std::string name_of(const element::ptr& img)
{
    const char* alt = img->get_attr("alt");
    return alt_name(squeezed(alt ? alt : ""));
}

// The names of the icons under `el`, by the table each sits in.
void icon_names(const element::ptr& el, std::map<const element*, std::vector<std::string>>& out)
{
    if(el->is_comment() || el->css().get_display() == display_none) return;
    if(tag_is(el, "img") && sized_icon(el))
        if(std::string name = name_of(el); !name.empty()) out[table_of(el).get()].push_back(name);
    for(auto& child : el->children()) icon_names(child, out);
}

} // namespace

void ImageLabels::label(const element::ptr& root, int cols)
{
    label_widths = LabelWidths(cols);
    redundant = redundant_links(root);
    shown_text(root, mail_text);
    mail_text = squeezed(mail_text);
    std::map<const element*, std::vector<std::string>> names;
    icon_names(root, names);
    for(auto& [table, in_table] : names)
        if(std::string prefix = shared_prefix(in_table); !prefix.empty()) prefixes.emplace(table, prefix);
    walk(root);
}

void ImageLabels::walk(const element::ptr& el)
{
    for(auto& child : el->children()) walk(child);
    if(!tag_is(el, "img")) return;
    if(picture(el)) pictured.push_back(el);
    if(el->css().get_display() != display_none) label_one(el);
}

// The name of `img` as its row of icons shows it: each names its sender,
// and `Kraken on X` is `X`.
std::string ImageLabels::named(const element::ptr& img, bool icon) const
{
    std::string name = name_of(img);
    auto shared = prefixes.find(table_of(img).get());
    if(!icon || shared == prefixes.end() || name.compare(0, shared->second.size(), shared->second) != 0) return name;
    return name.substr(shared->second.size());
}

// What an image shows, before any cut to its box; empty when it goes.
std::string ImageLabels::text_for(const element::ptr& el, bool icon, const element::ptr& link, bool sole)
{
    std::string alt = name_of(el);
    // An icon, or a file's name for an alt, decorates the text beside it.
    bool decorates = !sole && (icon || file_word(alt));
    if(invisible(el) || decorates || said_already(el, alt, mail_text)) return "";
    if(!alt.empty() || !sole || icon) return named(el, icon);
    if(labelled_links.count(link.get()) || redundant.count(link.get())) return "";
    labelled_links.insert(link.get());
    return "[" + link_name(link->get_attr("href")) + "]";
}

void ImageLabels::label_one(const element::ptr& el)
{
    bool icon = sized_icon(el);
    element::ptr link = link_of(el);
    std::string link_text;
    if(link) shown_text(link, link_text);
    bool sole = link && squeezed(link_text).empty();
    std::string label = text_for(el, icon, link, sole);
    if(!label_widths.fit(el, cells(label.c_str()), icon, sole))
    {
        el->css().set_display(display_none);
        return;
    }

    // Each image gets a src of its own: one icon file carries many alts.
    std::string key = "postal:image/" + std::to_string(labels.size());
    el->set_attr("src", key.c_str());
    el->parse_attributes();
    labels.emplace(key, label);
}

const std::string* ImageLabels::of(const std::string& src) const
{
    auto it = labels.find(src);
    return it == labels.end() ? nullptr : &it->second;
}
} // namespace cg
