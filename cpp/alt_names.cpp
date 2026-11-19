#include "alt_names.h"

#include <algorithm>
#include <array>
#include <cctype>

namespace cg
{
namespace
{
constexpr std::array PICTURE_NOUNS = {"icon", "image", "img", "logo", "picture", "photo", "banner"};
constexpr std::array ARTICLES = {"a", "an", "the", "some"};

template <size_t N> bool among(std::string word, const std::array<const char*, N>& words)
{
    for(auto& ch : word) ch = char(std::tolower(static_cast<unsigned char>(ch)));
    return std::find(words.begin(), words.end(), word) != words.end();
}
} // namespace

std::string alt_name(const std::string& alt)
{
    size_t cut = alt.find_last_of(' ');
    std::string last = alt.substr(cut == std::string::npos ? 0 : cut + 1);
    if(!among(last, PICTURE_NOUNS)) return alt;
    std::string rest = cut == std::string::npos ? "" : alt.substr(0, cut);
    return rest.find(' ') == std::string::npos && among(rest, ARTICLES) ? "" : rest;
}
} // namespace cg
