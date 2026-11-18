// The terminal cell in CSS px, and how lengths and boxes land on cells.
// See docs/adr/0006-rendered-view-engine.md.
#ifndef CG_GEOMETRY_H
#define CG_GEOMETRY_H

#include <cmath>

namespace cg
{
const float CW = 8, CH = 16;

// The cells whose centre lies in [a, a + len). Text rows use it too, so a
// line and the box around it round the same way.
inline void span(float a, float len, float unit, int& first, int& last)
{
    first = int(std::ceil(a / unit - 0.5f));
    last  = int(std::ceil((a + len) / unit - 0.5f)) - 1;
}

inline int first_cell(float a, float unit) { return int(std::ceil(a / unit - 0.5f)); }

// Whole rows. Half a row rounds up above a box and down below it, so the
// 8px under one paragraph and the 8px over the next make one row together.
inline float rows_above(float px) { return std::copysign(std::floor(std::fabs(px) / CH + 0.5f) * CH, px); }

inline float rows_below(float px) { return std::copysign(std::ceil(std::fabs(px) / CH - 0.5f) * CH, px); }

inline float columns(float px) { return std::copysign(std::floor(std::fabs(px) / CW + 0.5f) * CW, px); }
} // namespace cg

#endif
