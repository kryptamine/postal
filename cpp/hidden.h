// Elements CSS hides without `display: none`, such as email preheaders.
#ifndef CG_HIDDEN_H
#define CG_HIDDEN_H

#include <litehtml.h>

namespace cg
{
// Turns elements hidden by visibility, zero opacity or collapsed overflow
// into `display: none`, so layout and image passes both ignore them.
void hide_invisible(const litehtml::element::ptr& root);

// Whether a pass reading the mail's text skips `el`: a comment, an element
// not displayed, or text under 4 px or in transparent ink. Hiding such text
// before layout instead would leave its empty line boxes as blank rows.
bool unseen(const litehtml::element::ptr& el);
} // namespace cg

#endif
