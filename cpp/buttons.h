// Buttons found and laid out one row high, before the snap.
#ifndef CG_BUTTONS_H
#define CG_BUTTONS_H

#include "contents.h"

#include <litehtml.h>

#include <vector>

namespace cg
{
// Every button under `root` loses its padding, height and borders: its
// label is its one row. One drawn as a border alone gets a fill in the
// border's colour, so every button has one shape. Returns the buttons.
std::vector<litehtml::element::ptr> flatten_buttons(const litehtml::element::ptr& root, const Contents& contents);
} // namespace cg

#endif
