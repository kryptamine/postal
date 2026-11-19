#include "word_breaks.h"

#include "text.h"

#include <cstring>

namespace cg
{
namespace
{
const int WHOLE = 16, LEAST = 8, MOST = 24;
}

std::vector<std::string> breakable(const std::string& word)
{
    if(cells(word.c_str()) <= WHOLE) return {word};
    std::vector<std::string> pieces;
    std::string              piece;
    int                      width = 0;
    for(const char* s = word.c_str(); *s;)
    {
        const char* start = s;
        uint32_t    cp;
        utf8_next(s, cp);
        int w = cp_width(cp);
        if(width + w > MOST)
        {
            pieces.push_back(piece);
            piece.clear();
            width = 0;
        }
        piece.append(start, s);
        width += w;
        if(width >= LEAST && cp < 128 && std::strchr("/?&=._-:,;@%+#", int(cp)))
        {
            pieces.push_back(piece);
            piece.clear();
            width = 0;
        }
    }
    if(!piece.empty()) pieces.push_back(piece);
    return pieces;
}
} // namespace cg
