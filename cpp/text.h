// UTF-8 text measured in terminal cells.
#ifndef CG_TEXT_H
#define CG_TEXT_H

#include <cstdint>
#include <string>

namespace cg
{
// Reads one code point at `s` and steps past it; a broken sequence is U+FFFD.
void utf8_next(const char*& s, uint32_t& cp);
// 0 for a combining or invisible code point, 2 for a wide one, else 1.
int cp_width(uint32_t cp);
int cells(const char* text);
// `text` cut to `cols` cells, its last one an ellipsis when it was cut.
std::string fit(const std::string& text, int cols);
// Runs of whitespace, NBSP among them, become one space, none at the ends.
std::string squeezed(const std::string& s);
} // namespace cg

#endif
