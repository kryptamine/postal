// Questions about the mail's elements that the passes before layout share.
#ifndef CG_DOM_H
#define CG_DOM_H

#include <litehtml.h>

#include <string>

namespace cg
{
bool tag_is(const litehtml::element::ptr& el, const char* tag);
// The text a reader sees: litehtml's `get_text` takes in comments too, and
// mail hides whole Outlook tables in conditional ones.
void shown_text(const litehtml::element::ptr& el, std::string& out);
// The nearest link around `el`, not `el` itself; null outside one.
litehtml::element::ptr link_of(const litehtml::element::ptr& el);
// The nearest table around `el`; null outside one.
litehtml::element::ptr table_of(const litehtml::element::ptr& el);
} // namespace cg

#endif
