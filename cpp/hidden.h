// Elements CSS hides without `display: none`, such as email preheaders.
#ifndef CG_HIDDEN_H
#define CG_HIDDEN_H

#include <litehtml.h>

namespace cg
{
// Turns elements hidden by visibility, zero opacity or collapsed overflow
// into `display: none`, so layout and image passes both ignore them.
void hide_invisible(const litehtml::element::ptr& root);

// Whether a text node draws nothing: font under 4 px or transparent ink.
bool text_hidden(const litehtml::element::ptr& el);
} // namespace cg

#endif
