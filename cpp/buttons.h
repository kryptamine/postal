// Buttons laid out one row high, before the snap.
#ifndef CG_BUTTONS_H
#define CG_BUTTONS_H

#include "contents.h"

#include <litehtml.h>

namespace cg
{
// Every button under `root` loses its padding, height and borders: its
// label is its one row, and the fill round it rows of nothing but fill.
void flatten_buttons(const litehtml::element::ptr& root, const Contents& contents);
} // namespace cg

#endif
