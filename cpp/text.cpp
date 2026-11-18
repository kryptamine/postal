#include "text.h"

#include <cstring>

namespace cg
{
void utf8_next(const char*& s, uint32_t& cp)
{
    auto u = reinterpret_cast<const unsigned char*>(s);
    if(u[0] < 0x80) { cp = u[0]; s += 1; }
    else if((u[0] >> 5) == 6 && u[1]) { cp = ((u[0] & 31u) << 6) | (u[1] & 63u); s += 2; }
    else if((u[0] >> 4) == 14 && u[1] && u[2]) { cp = ((u[0] & 15u) << 12) | ((u[1] & 63u) << 6) | (u[2] & 63u); s += 3; }
    else if(u[1] && u[2] && u[3]) { cp = ((u[0] & 7u) << 18) | ((u[1] & 63u) << 12) | ((u[2] & 63u) << 6) | (u[3] & 63u); s += 4; }
    else { cp = 0xFFFD; s += 1; }
}

int cp_width(uint32_t cp)
{
    if((cp >= 0x300 && cp <= 0x36F) || (cp >= 0x200B && cp <= 0x200F) || (cp >= 0xFE00 && cp <= 0xFE0F) ||
       cp == 0xFEFF || cp == 0x34F || cp == 0xAD || (cp >= 0xE0000 && cp <= 0xE007F))
        return 0;
    if((cp >= 0x1100 && cp <= 0x115F) || (cp >= 0x2E80 && cp <= 0xA4CF) || (cp >= 0xAC00 && cp <= 0xD7A3) ||
       (cp >= 0xF900 && cp <= 0xFAFF) || (cp >= 0xFE30 && cp <= 0xFE4F) || (cp >= 0xFF00 && cp <= 0xFF60) ||
       (cp >= 0xFFE0 && cp <= 0xFFE6) || (cp >= 0x1F300 && cp <= 0x1F64F) || (cp >= 0x1F680 && cp <= 0x1F6FF) ||
       (cp >= 0x1F900 && cp <= 0x1FAFF) || (cp >= 0x20000 && cp <= 0x3FFFD))
        return 2;
    return 1;
}

int cells(const char* text)
{
    int w = 0;
    for(const char* s = text; *s;)
    {
        uint32_t cp;
        utf8_next(s, cp);
        w += cp_width(cp);
    }
    return w;
}

std::string fit(const std::string& text, int cols)
{
    int         w = 0;
    const char* s = text.c_str();
    const char* cut = s;
    while(*s)
    {
        uint32_t cp;
        utf8_next(s, cp);
        w += cp_width(cp);
        if(w > cols) return std::string(text.c_str(), cut) + "…";
        if(w <= cols - 1) cut = s;
    }
    return text;
}

std::string loosely_squeezed(const std::string& s)
{
    std::string out;
    bool        space = false;
    for(size_t i = 0; i < s.size(); i++)
    {
        bool nbsp = s.compare(i, 2, "\xC2\xA0") == 0;
        if(nbsp || std::strchr(" \t\r\n\f", s[i]))
        {
            space = true;
            i += nbsp;
            continue;
        }

        if(space) out += ' ';
        space = false;
        out += s[i];
    }
    if(space) out += ' ';
    return out;
}

std::string squeezed(const std::string& s)
{
    std::string out = loosely_squeezed(s);
    size_t      start = out.compare(0, 1, " ") == 0, end = out.size();
    if(end > start && out[end - 1] == ' ') end--;
    return out.substr(start, end - start);
}
} // namespace cg
