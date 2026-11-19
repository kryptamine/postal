// Text alignment that reads badly in one-cell glyphs, set left.
#ifndef CG_ALIGNMENT_H
#define CG_ALIGNMENT_H

#include <litehtml.h>

namespace cg
{
// After layout: a centred block whose words wrap onto three lines or more
// is a paragraph, hard to read from a ragged left edge. Each is set left;
// true when one was, and the layout must run again.
bool left_align_paragraphs(const litehtml::element::ptr& root);
} // namespace cg

#endif
