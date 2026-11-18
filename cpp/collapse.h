// A box shrunk to the lines it holds, before the snap.
#ifndef CG_COLLAPSE_H
#define CG_COLLAPSE_H

#include <litehtml.h>

namespace cg
{
// `el` and everything in it lose their vertical padding, their borders and
// any height set: what is left is the rows of their text.
void collapse(const litehtml::element::ptr& el);
} // namespace cg

#endif
