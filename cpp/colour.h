// Mail colours as the grid packs them: 0x01RRGGBB, 0 for none.
#ifndef CG_COLOUR_H
#define CG_COLOUR_H

#include <litehtml.h>

#include <cstdint>

namespace cg
{
uint32_t rgb(const litehtml::web_color& c);
// `over` at opacity `a` on `under`; mail colours assume a white page.
uint32_t blend(uint32_t under, uint32_t over, float a);
// `c` laid on `under`, blended by its own alpha.
uint32_t over(uint32_t under, const litehtml::web_color& c);
// Light enough to be the page, which the terminal's background replaces.
bool page_light(uint32_t c);
bool grey(uint32_t c);
// No channel apart by more than a tenth of its range.
bool alike(uint32_t a, uint32_t b);
} // namespace cg

#endif
