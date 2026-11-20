// The small print and the headings, told by font size: every glyph is one
// cell, so size shows only as these two marks.
#ifndef CG_FONT_SIZES_H
#define CG_FONT_SIZES_H

#include "grid.h"

#include <litehtml.h>

#include <cstdint>

namespace cg
{
// The size the mail's body is set in: the one most of its letters of 13 px
// or more have, as legal lines can outnumber the body's.
float body_size(const litehtml::element::ptr& root);
// CG_SMALL, CG_TITLE or none for text `size` px in a mail set in `body`.
uint32_t size_flags(float size, float body);
// After the links are marked: CG_SMALL stays only on the small print, the
// blocks under the last row of the body's words that are no link. Above
// it, small text is a caption or a card's copy and reads as the body.
void keep_small_print(Grid& grid);
} // namespace cg

#endif
