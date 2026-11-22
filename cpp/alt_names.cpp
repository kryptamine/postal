#include "alt_names.h"

#include <algorithm>
#include <array>
#include <cctype>
#include <string_view>

namespace cg
{
namespace
{
constexpr std::array PICTURE_NOUNS = {"icon", "image", "img", "logo", "picture", "photo", "banner", "illustration"};
// What a picture of a person is called after their name: `Ann's profile`.
constexpr std::array PORTRAIT_NOUNS = {"profile", "avatar"};
constexpr std::array POSSESSIVES = {"'s", "\u2019s"};
constexpr std::array ARTICLES = {"a", "an", "the", "some"};

template <size_t N> bool among(std::string word, const std::array<const char*, N>& words)
{
    for(auto& ch : word) ch = char(std::tolower(static_cast<unsigned char>(ch)));
    return std::find(words.begin(), words.end(), word) != words.end();
}

// `Ann Lee's profile` names `Ann Lee`, the person the picture shows.
std::string portrayed(const std::string& name)
{
    size_t cut = name.find_last_of(' ');
    if(cut == std::string::npos || !among(name.substr(cut + 1), PORTRAIT_NOUNS)) return name;
    std::string owner = name.substr(0, cut);
    for(std::string_view s : POSSESSIVES)
        if(owner.size() > s.size() && owner.compare(owner.size() - s.size(), s.size(), s) == 0)
            return owner.substr(0, owner.size() - s.size());
    return owner;
}
} // namespace

std::string alt_name(const std::string& alt)
{
    size_t cut = alt.find_last_of(' ');
    std::string last = alt.substr(cut == std::string::npos ? 0 : cut + 1);
    if(!among(last, PICTURE_NOUNS)) return alt;
    std::string rest = cut == std::string::npos ? "" : alt.substr(0, cut);
    if(rest.find(' ') == std::string::npos && among(rest, ARTICLES)) return "";
    return portrayed(rest);
}

bool description(const std::string& alt) { return std::count(alt.begin(), alt.end(), ' ') >= 5; }

bool file_name(const std::string& alt)
{
    return alt.find(' ') == std::string::npos && alt.find('_') != std::string::npos;
}

std::string file_words(std::string alt, bool first)
{
    if(first) return alt.substr(0, alt.find('_'));
    std::replace(alt.begin(), alt.end(), '_', ' ');
    return alt;
}

bool file_word(const std::string& alt)
{
    return !alt.empty() && std::all_of(alt.begin(), alt.end(), [](unsigned char c) { return std::islower(c) != 0; });
}
} // namespace cg
