// Text alignment that reads badly in one-cell glyphs, set left.
#ifndef CG_ALIGNMENT_H
#define CG_ALIGNMENT_H

#include <litehtml.h>

namespace cg
{
// Before layout: justified text is set left, since a line of one-cell
// glyphs is padded with whole cells, and so is centred text in a list
// item, which stood apart from its marker.
void left_align_spread(const litehtml::element::ptr& root);
// Before layout: a block or table under `align=center` or `<center>`, on
// its parent or further up, stands centred in its container, as in Blink.
void align_blocks(const litehtml::element::ptr& root);
// After layout: a centred block whose words wrap onto three lines or more
// is a paragraph, hard to read from a ragged left edge. Each is set left;
// true when one was, and the layout must run again.
bool left_align_paragraphs(const litehtml::element::ptr& root);
} // namespace cg

#endif
