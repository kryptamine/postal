#include "hidden.h"

#include <algorithm>
#include <cctype>
#include <string>
#include <string_view>

using namespace litehtml;

namespace cg
{
namespace
{
std::string_view trimmed(std::string_view s)
{
    while(!s.empty() && std::isspace(static_cast<unsigned char>(s.front()))) s.remove_prefix(1);
    while(!s.empty() && std::isspace(static_cast<unsigned char>(s.back()))) s.remove_suffix(1);
    return s;
}

bool eq_ci(std::string_view a, std::string_view b)
{
    if(a.size() != b.size()) return false;
    for(size_t i = 0; i < a.size(); i++)
        if(std::tolower(static_cast<unsigned char>(a[i])) != std::tolower(static_cast<unsigned char>(b[i])))
            return false;
    return true;
}

bool zero_value(std::string_view value)
{
    value = trimmed(value);
    if(size_t bang = value.find('!'); bang != std::string_view::npos) value = trimmed(value.substr(0, bang));
    while(!value.empty() && (std::isalpha(static_cast<unsigned char>(value.back())) || value.back() == '%'))
        value.remove_suffix(1);
    value = trimmed(value);
    return !value.empty() && value.find('0') != std::string_view::npos &&
           std::all_of(value.begin(), value.end(), [](char c) { return c == '0' || c == '.'; });
}

// litehtml does not parse `opacity`, so inline `opacity: 0` preheaders
// are checked from the style attribute directly.
bool zero_opacity(const element::ptr& el)
{
    const char* raw = el->get_attr("style");
    if(!raw) return false;
    std::string_view style(raw);
    while(!style.empty())
    {
        size_t semi = style.find(';');
        std::string_view decl = semi == std::string_view::npos ? style : style.substr(0, semi);
        style = semi == std::string_view::npos ? std::string_view() : style.substr(semi + 1);
        size_t colon = decl.find(':');
        if(colon == std::string_view::npos) continue;
        if(eq_ci(trimmed(decl.substr(0, colon)), "opacity") && zero_value(decl.substr(colon + 1))) return true;
    }
    return false;
}

bool has_border(const css_borders& b)
{
    for(const css_border* e : {&b.top, &b.bottom, &b.left, &b.right})
        if(e->width.val() > 0 && e->style != border_style_none && e->style != border_style_hidden && e->color.alpha > 0)
            return true;
    return false;
}

// A box clipped to zero max-height shows nothing; a bordered one is a rule.
bool collapsed_clip(const css_properties& css)
{
    const css_length& max_h = css.get_max_height();
    return css.get_overflow() == overflow_hidden && !max_h.is_predefined() && max_h.val() <= 0 &&
           !has_border(css.get_borders());
}

bool has_visible_descendant(const element::ptr& el)
{
    for(auto& child : el->children())
    {
        if(child->is_comment() || child->is_text() || child->css().get_display() == display_none) continue;
        if(child->css().get_visibility() == visibility_visible || has_visible_descendant(child)) return true;
    }
    return false;
}
} // namespace

void hide_invisible(const element::ptr& root)
{
    if(root->is_comment() || root->is_text() || root->css().get_display() == display_none) return;
    css_properties& css = root->css();
    bool invisible = css.get_visibility() != visibility_visible && !has_visible_descendant(root);
    if(invisible || zero_opacity(root) || collapsed_clip(css))
    {
        css.set_display(display_none);
        return;
    }

    for(auto& child : root->children()) hide_invisible(child);
}

bool text_hidden(const element::ptr& el)
{
    if(!el->is_text()) return false;
    if(float(el->css().get_font_metrics().font_size) < 4.f) return true;
    element::ptr parent = el->parent();
    return parent && parent->css().get_color().alpha == 0;
}
} // namespace cg
