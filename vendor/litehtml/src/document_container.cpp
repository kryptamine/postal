#include "utf8_strings.h"
#include "document_container.h"

void litehtml::document_container::split_text(const char* text, const std::function<void(const char*)>& on_word,
                                              const std::function<void(const char*)>& on_space)
{
    std::u32string str;
    // Postal: a run of spaces is one element: a collapsible one draws as one
    // space, a `pre` one as its text. A line feed or a tab stays alone.
    std::u32string spaces;
    auto end_spaces = [&]() {
        if(!spaces.empty())
        {
            on_space(utf32_to_utf8(spaces));
            spaces.clear();
        }
    };
    std::u32string str_in = static_cast<const char32_t*>(utf8_to_utf32(text));
    for(auto c : str_in)
    {
        if(c <= ' ' && (c == ' ' || c == '\t' || c == '\n' || c == '\r' || c == '\f'))
        {
            if(!str.empty())
            {
                on_word(utf32_to_utf8(str));
                str.clear();
            }
            if(c == ' ')
            {
                spaces += c;
                continue;
            }
            end_spaces();
            str += c;
            on_space(utf32_to_utf8(str));
            str.clear();
            continue;
        }
        end_spaces();
        // CJK character range
        if(c >= 0x4E00 && c <= 0x9FCC)
        {
            if(!str.empty())
            {
                on_word(utf32_to_utf8(str));
                str.clear();
            }
            str += c;
            on_word(utf32_to_utf8(str));
            str.clear();
        } else
        {
            str += c;
        }
    }
    if(!str.empty())
    {
        on_word(utf32_to_utf8(str));
    }
    end_spaces();
}
