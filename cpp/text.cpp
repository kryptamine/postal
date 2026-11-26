#include "text.h"

#include <cstring>

namespace cg
{
namespace
{
bool bmp_wide_emoji(uint32_t cp)
{
    return (cp >= 0x231A && cp <= 0x231B) || (cp >= 0x23E9 && cp <= 0x23EC) || cp == 0x23F0 || cp == 0x23F3 ||
           (cp >= 0x25FD && cp <= 0x25FE) || (cp >= 0x2614 && cp <= 0x2615) || (cp >= 0x2648 && cp <= 0x2653) ||
           cp == 0x267F || cp == 0x2693 || cp == 0x26A1 || (cp >= 0x26AA && cp <= 0x26AB) ||
           (cp >= 0x26BD && cp <= 0x26BE) || (cp >= 0x26C4 && cp <= 0x26C5) || cp == 0x26CE || cp == 0x26D4 ||
           cp == 0x26EA || (cp >= 0x26F2 && cp <= 0x26F3) || cp == 0x26F5 || cp == 0x26FA || cp == 0x26FD ||
           cp == 0x2705 || (cp >= 0x270A && cp <= 0x270B) || cp == 0x2728 || cp == 0x274C || cp == 0x274E ||
           (cp >= 0x2753 && cp <= 0x2755) || cp == 0x2757 || (cp >= 0x2795 && cp <= 0x2797) || cp == 0x27B0 ||
           cp == 0x27BF || (cp >= 0x2B1B && cp <= 0x2B1C) || cp == 0x2B50 || cp == 0x2B55;
}

bool regional_indicator(uint32_t cp) { return cp >= 0x1F1E6 && cp <= 0x1F1FF; }

void utf8_push(std::string& out, uint32_t cp)
{
    if(cp < 0x80) out.push_back(char(cp));
    else if(cp < 0x800)
    {
        out.push_back(char(0xC0 | (cp >> 6)));
        out.push_back(char(0x80 | (cp & 0x3F)));
    }
    else if(cp < 0x10000)
    {
        out.push_back(char(0xE0 | (cp >> 12)));
        out.push_back(char(0x80 | ((cp >> 6) & 0x3F)));
        out.push_back(char(0x80 | (cp & 0x3F)));
    }
    else
    {
        out.push_back(char(0xF0 | (cp >> 18)));
        out.push_back(char(0x80 | ((cp >> 12) & 0x3F)));
        out.push_back(char(0x80 | ((cp >> 6) & 0x3F)));
        out.push_back(char(0x80 | (cp & 0x3F)));
    }
}

uint32_t to_upper(uint32_t cp)
{
    if(cp >= 'a' && cp <= 'z') return cp - 32;
    if(cp >= 0x00E0 && cp <= 0x00FE && cp != 0x00F7) return cp - 32;
    if(cp >= 0x0430 && cp <= 0x044F) return cp - 32;
    if(cp >= 0x0450 && cp <= 0x045F) return cp - 80;
    return cp;
}

uint32_t to_lower(uint32_t cp)
{
    if(cp >= 'A' && cp <= 'Z') return cp + 32;
    if(cp >= 0x00C0 && cp <= 0x00DE && cp != 0x00D7) return cp + 32;
    if(cp >= 0x0410 && cp <= 0x042F) return cp + 32;
    if(cp >= 0x0400 && cp <= 0x040F) return cp + 80;
    return cp;
}

bool is_letter(uint32_t cp)
{
    return (cp >= 'A' && cp <= 'Z') || (cp >= 'a' && cp <= 'z') ||
           (cp >= 0x00C0 && cp <= 0x00FF && cp != 0x00D7 && cp != 0x00F7) || (cp >= 0x0400 && cp <= 0x045F);
}
} // namespace

void utf8_next(const char*& s, uint32_t& cp)
{
    auto u = reinterpret_cast<const unsigned char*>(s);
    if(u[0] < 0x80)
    {
        cp = u[0];
        s += 1;
    }
    else if((u[0] >> 5) == 6 && u[1])
    {
        cp = ((u[0] & 31u) << 6) | (u[1] & 63u);
        s += 2;
    }
    else if((u[0] >> 4) == 14 && u[1] && u[2])
    {
        cp = ((u[0] & 15u) << 12) | ((u[1] & 63u) << 6) | (u[2] & 63u);
        s += 3;
    }
    else if(u[1] && u[2] && u[3])
    {
        cp = ((u[0] & 7u) << 18) | ((u[1] & 63u) << 12) | ((u[2] & 63u) << 6) | (u[3] & 63u);
        s += 4;
    }
    else
    {
        cp = 0xFFFD;
        s += 1;
    }
}

int cp_width(uint32_t cp)
{
    if((cp >= 0x0300 && cp <= 0x036F) || (cp >= 0x1AB0 && cp <= 0x1AFF) || (cp >= 0x1DC0 && cp <= 0x1DFF) ||
       (cp >= 0x200B && cp <= 0x200F) || (cp >= 0x202A && cp <= 0x202E) || (cp >= 0x2060 && cp <= 0x206F) ||
       (cp >= 0x20D0 && cp <= 0x20FF) || (cp >= 0xFE00 && cp <= 0xFE0F) || (cp >= 0xFE20 && cp <= 0xFE2F) ||
       cp == 0xFEFF || cp == 0x034F || cp == 0x00AD || (cp >= 0x1F3FB && cp <= 0x1F3FF) ||
       (cp >= 0xE0000 && cp <= 0xE007F) || (cp >= 0xE0100 && cp <= 0xE01EF))
        return 0;
    if(bmp_wide_emoji(cp) || (cp >= 0x1100 && cp <= 0x115F) || (cp >= 0x2E80 && cp <= 0xA4CF) ||
       (cp >= 0xAC00 && cp <= 0xD7A3) || (cp >= 0xF900 && cp <= 0xFAFF) || (cp >= 0xFE30 && cp <= 0xFE4F) ||
       (cp >= 0xFF00 && cp <= 0xFF60) || (cp >= 0xFFE0 && cp <= 0xFFE6) || (cp >= 0x1F300 && cp <= 0x1F64F) ||
       (cp >= 0x1F680 && cp <= 0x1F6FF) || (cp >= 0x1F900 && cp <= 0x1FAFF) || (cp >= 0x20000 && cp <= 0x3FFFD))
        return 2;
    return 1;
}

void next_cluster(const char*& s, uint32_t& lead, int& width)
{
    utf8_next(s, lead);
    width = cp_width(lead);
    if(width == 0) return;

    if(regional_indicator(lead) && *s)
    {
        const char* peek = s;
        uint32_t second = 0;
        utf8_next(peek, second);
        if(regional_indicator(second))
        {
            s = peek;
            width = 2;
        }
    }

    while(*s)
    {
        const char* peek = s;
        uint32_t next = 0;
        utf8_next(peek, next);
        if(next == 0xFE0F && lead >= 0x2000 && lead <= 0x2BFF)
        {
            s = peek;
            width = 2;
        }
        else if(next == 0x200D && *peek)
        {
            s = peek;
            uint32_t joined = 0;
            utf8_next(s, joined);
            width = 2;
        }
        else if(cp_width(next) == 0) s = peek;
        else break;
    }
}

int cells(const char* text)
{
    int w = 0;
    for(const char* s = text; *s;)
    {
        uint32_t lead = 0;
        int cw = 0;
        next_cluster(s, lead, cw);
        w += cw;
    }

    return w;
}

std::string fit(const std::string& text, int cols)
{
    int w = 0;
    const char* s = text.c_str();
    const char* cut = s;
    while(*s)
    {
        uint32_t lead = 0;
        int cw = 0;
        next_cluster(s, lead, cw);
        w += cw;
        if(w > cols) return std::string(text.c_str(), cut) + "…";
        if(w <= cols - 1) cut = s;
    }

    return text;
}

std::string loosely_squeezed(const std::string& s)
{
    std::string out;
    bool space = false;
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
    size_t start = out.compare(0, 1, " ") == 0, end = out.size();
    if(end > start && out[end - 1] == ' ') end--;
    return out.substr(start, end - start);
}

void transform_case(std::string& text, Case mode)
{
    std::string out;
    out.reserve(text.size());
    bool in_word = false;
    for(const char* s = text.c_str(); *s;)
    {
        uint32_t cp = 0;
        utf8_next(s, cp);
        if(mode == Case::upper) cp = to_upper(cp);
        else if(mode == Case::lower) cp = to_lower(cp);
        else
        {
            bool letter = is_letter(cp);
            if(letter && !in_word) cp = to_upper(cp);
            in_word = letter;
        }

        utf8_push(out, cp);
    }

    text = std::move(out);
}

std::string lower_utf8(const std::string& s)
{
    std::string out = s;
    transform_case(out, Case::lower);
    return out;
}
} // namespace cg
