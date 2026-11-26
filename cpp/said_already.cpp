#include "said_already.h"

#include "dom.h"
#include "text.h"

#include <algorithm>

using namespace litehtml;

namespace cg
{
namespace
{
// An alt this long is a title rather than a name.
const int TITLE_CELLS = 16;

// The levels above an image its caption or its row's text is looked for in.
const int NEARBY = 5;

// The nearest text around `el` already says `alt`, as a caption or a
// name beside an avatar or a status icon does.
bool repeats_nearby(const element::ptr& el, const std::string& alt)
{
    if(alt.empty()) return false;
    auto p = el->parent();
    for(int depth = 0; p && depth < NEARBY; depth++, p = p->parent())
    {
        // A table's other rows are other content, not this image's row.
        if(tag_is(p, "table") || tag_is(p, "tbody") || tag_is(p, "thead")) return false;
        std::string text;
        shown_text(p, text);
        text = squeezed(text);
        // Past a caption's or a table row's length, the text is the mail's.
        if(!text.empty()) return text.size() <= 200 && lower_utf8(text).find(alt) != std::string::npos;
    }

    return false;
}

// The first words among the siblings of `at`, after it or before it;
// empty when they show none.
std::string words_beside(const element::ptr& at, bool after)
{
    const auto& around = at->parent()->children();
    auto self = std::find(around.begin(), around.end(), at);
    std::string text;
    if(after)
        for(auto next = std::next(self); next != around.end() && squeezed(text).empty(); ++next)
            shown_text(*next, text);
    else
        for(auto before = self; before != around.begin() && squeezed(text).empty();) shown_text(*--before, text);
    return squeezed(text);
}

// The words beside `el`, after it or else before it, at the nearest level
// up to its table that has any, are `alt` and nothing more: a caption in
// the next row of a card.
bool captioned(const element::ptr& el, const std::string& alt)
{
    element::ptr at = el;
    for(int depth = 0; at->parent() && depth < NEARBY && !tag_is(at, "table"); depth++, at = at->parent())
    {
        std::string beside = words_beside(at, true);
        if(beside.empty()) beside = words_beside(at, false);
        if(!beside.empty()) return lower_utf8(beside) == alt;
    }

    return false;
}

// A title the mail shows as text is a card's picture beside its caption,
// however far the markup puts it; a name is looked for nearby only.
bool titles(const std::string& alt, const std::string& mail_text)
{
    return cells(alt.c_str()) >= TITLE_CELLS && mail_text.find(alt) != std::string::npos;
}
} // namespace

bool said_already(const element::ptr& img, const std::string& alt, const std::string& mail_text)
{
    std::string low = lower_utf8(alt);
    return !low.empty() && (repeats_nearby(img, low) || captioned(img, low) || titles(low, mail_text));
}
} // namespace cg
