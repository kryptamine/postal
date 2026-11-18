#include "contents.h"

#include "dom.h"
#include "text.h"

#include <utility>
#include <vector>

using namespace litehtml;

namespace cg
{
namespace
{
const element* block_of(const element::ptr& text)
{
    auto block = text->parent();
    while(block && block->css().get_display() == display_inline) block = block->parent();
    return block.get();
}

void cap(Holds& h)
{
    if(!h.over && cells(h.words().c_str()) > WORDS_KEPT)
    {
        h.over = true;
        h.text.clear();
    }
}

void join(Holds& h, const Holds& child)
{
    h.over = h.over || child.over;
    h.text = h.over ? std::string() : loosely_squeezed(h.text + child.text);
    if(child.blocks || (h.block && child.block && h.block != child.block)) h.blocks = true;
    else if(!h.block) h.block = child.block;
    cap(h);
}
} // namespace

std::string Holds::words() const { return squeezed(text); }

Contents::Contents(const element::ptr& root) { gather(root); }

const Holds& Contents::of(const element* el) const
{
    static const Holds nothing;
    auto               it = holds.find(el);
    return it == holds.end() ? nothing : it->second;
}

Holds Contents::gather(const element::ptr& el)
{
    Holds h;
    if(el->is_comment() || el->css().get_display() == display_none) return holds[el.get()] = h;
    if(el->is_text())
    {
        std::string raw;
        el->get_text(raw);
        h.text = loosely_squeezed(raw);
        if(!h.words().empty()) h.block = block_of(el);
        cap(h);
        return holds[el.get()] = h;
    }

    std::vector<std::pair<element::ptr, Holds>> children;
    for(auto& child : el->children())
    {
        children.emplace_back(child, gather(child));
        join(h, children.back().second);
    }

    // The first child to show all the words carries them down.
    std::string words = h.words();
    for(auto& [child, inner] : children)
        if(!h.over && !words.empty() && inner.words() == words)
        {
            h.linked = tag_is(child, "a") || inner.linked;
            h.filled = child->css().get_bg().m_color.alpha || inner.filled;
            break;
        }
    return holds[el.get()] = h;
}
} // namespace cg
