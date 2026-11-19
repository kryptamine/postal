// The widths a mail fixes, let go for a pane it does not fit.
#ifndef CG_LOOSEN_H
#define CG_LOOSEN_H

#include <litehtml.h>

namespace cg
{
// Every box under `root` may shrink and wrap: no least width, no width in
// px but an image's, and no `nowrap`.
void loosen(const litehtml::element::ptr& root);
} // namespace cg

#endif
