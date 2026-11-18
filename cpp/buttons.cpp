#include "buttons.h"

#include "collapse.h"
#include "colour.h"
#include "dom.h"

using namespace litehtml;

namespace cg
{
namespace
{
// The innermost filled box round one line of a few words, all a link's, is
// a button; the sections round it are not. A page-light fill gives way to
// the terminal's, and the frame round its rows is the button's shape.
bool button(const element::ptr& el, const Holds& h)
{
    const web_color& bg = el->css().get_bg().m_color;
    if(!bg.alpha || page_light(rgb(bg))) return false;
    if(h.over || h.words().empty() || h.blocks || h.filled) return false;
    return tag_is(el, "a") || link_of(el) || h.linked;
}
} // namespace

void flatten_buttons(const element::ptr& root, const Contents& contents)
{
    // A slab three rows high reads as no button, and a border a cell thick
    // only adds a row to the fill that is its shape.
    if(button(root, contents.of(root.get()))) return collapse(root);
    for(auto& child : root->children()) flatten_buttons(child, contents);
}
} // namespace cg
