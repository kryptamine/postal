// UTF-8 text measured in terminal cells and transformed by case.
#ifndef CG_TEXT_H
#define CG_TEXT_H

#include <cstdint>
#include <string>

namespace cg
{
enum class Case : uint8_t
{
    upper,
    lower,
    capitalize,
};

// Reads one code point at `s` and steps past it; a broken sequence is U+FFFD.
void utf8_next(const char*& s, uint32_t& cp);
// 0 for a combining or invisible code point, 2 for a wide one, else 1.
int cp_width(uint32_t cp);
struct Cluster
{
    uint32_t lead;
    int width;
};

// Steps `s` past one terminal glyph cluster: its lead code point and cell
// width (0, 1 or 2).
Cluster next_cluster(const char*& s);
int cells(const char* text);
// `text` cut to `cols` cells, its last one an ellipsis when it was cut.
std::string fit(const std::string& text, int cols);
// Runs of whitespace, NBSP among them, become one space, none at the ends.
std::string squeezed(const std::string& s);
// As `squeezed`, keeping one space at an end with whitespace, so two pieces
// squeezed apart join as the whole would.
std::string loosely_squeezed(const std::string& s);
// `squeezed` and lowercased: text as compared for saying the same.
std::string folded(const std::string& s);
// ASCII, Latin-1 and Cyrillic case mapping in UTF-8.
void transform_case(std::string& text, Case mode);
} // namespace cg

#endif
