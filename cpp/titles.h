// A filled box holding an `h1`, a line or two of text and no link or
// picture is a cover band its logo and pictures left: a title, not a
// section. See docs/adr/0006-rendered-view-engine.md.
#ifndef CG_TITLES_H
#define CG_TITLES_H

#include "contents.h"

#include <litehtml.h>

namespace cg
{
// Takes the fill, borders, vertical padding, height and inner vertical
// margins off each such box, so its title stands as bold lines of text.
void unframe_titles(const litehtml::element::ptr& root, const Contents& contents);
} // namespace cg

#endif
