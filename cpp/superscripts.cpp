#include "superscripts.h"

#include "dom.h"
#include "text.h"

#include <litehtml/el_text.h>

#include <array>
#include <cctype>

using namespace litehtml;

namespace cg
{
namespace
{
bool digits(const std::string& text)
{
    if(text.empty()) return false;
    for(char c : text)
        if(!std::isdigit(static_cast<unsigned char>(c))) return false;
    return true;
}

bool in_sup(element::ptr el)
{
    for(; el; el = el->parent())
        if(tag_is(el, "sup")) return true;
    return false;
}

// Every word under `el` is digits in a `<sup>`; `any` once one is.
bool all_raised(const element::ptr& el, bool& any)
{
    if(el->is_comment() || el->css().get_display() == display_none) return true;
    if(el->is_text())
    {
        std::string text;
        el->get_text(text);
        text = squeezed(text);
        if(text.empty()) return true;
        any = true;
        return digits(text) && in_sup(el);
    }
    for(auto& child : el->children())
        if(!all_raised(child, any)) return false;
    return true;
}

std::string superscript(const std::string& digits)
{
    static constexpr std::array raised_digits = {"⁰", "¹", "²", "³", "⁴", "⁵", "⁶", "⁷", "⁸", "⁹"};
    std::string out;
    for(char c : digits) out += raised_digits[size_t(c - '0')];
    return out;
}

void raise(const element::ptr& el)
{
    for(auto& child : el->children())
    {
        std::string text;
        child->get_text(text);
        if(auto word = std::dynamic_pointer_cast<el_text>(child); word && digits(text))
            word->set_text(superscript(text));
        else raise(child);
    }
}
} // namespace

bool raised(const element::ptr& el)
{
    bool any = false;
    return all_raised(el, any) && any;
}

void raise_digits(const element::ptr& root)
{
    if(root->is_comment() || root->is_text()) return;
    if(tag_is(root, "sup") && raised(root)) return raise(root);
    for(auto& child : root->children()) raise_digits(child);
}
} // namespace cg
