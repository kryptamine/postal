// Buttons found and laid out one row high before the snap, and given one
// shape after the draw.
#ifndef CG_BUTTONS_H
#define CG_BUTTONS_H

#include "contents.h"
#include "grid.h"

#include <litehtml.h>

#include <vector>

namespace cg
{
// Every button under `root` loses its padding, height and borders: its
// label is its one row. One drawn as a border alone gets a fill in the
// border's colour, so every button has one shape. Returns the buttons.
std::vector<litehtml::element::ptr> flatten_buttons(const litehtml::element::ptr& root, const Contents& contents);
// After the draw: each of `buttons` is its label in bold with two cells of
// its fill at each side, whatever width and padding the mail gave it.
void shape_buttons(const std::vector<litehtml::element::ptr>& buttons, Grid& grid);
} // namespace cg

#endif
