#include "shared_prefix.h"

#include <algorithm>
#include <map>

namespace cg
{
namespace
{
// The word of `name` at `from` with the space after it; empty when the
// name ends with that word, which is then its whole name.
std::string word_at(const std::string& name, size_t from)
{
    size_t space = name.find(' ', from);
    return space == std::string::npos ? "" : name.substr(from, space + 1 - from);
}
} // namespace

std::string shared_prefix(const std::vector<std::string>& names)
{
    if(names.size() < 3) return "";
    std::string prefix;
    for(;;)
    {
        std::map<std::string, size_t> next;
        for(auto& name : names)
            if(name.compare(0, prefix.size(), prefix) == 0)
                if(std::string word = word_at(name, prefix.size()); !word.empty()) next[word]++;
        auto most = std::max_element(next.begin(), next.end(),
                                     [](const auto& a, const auto& b) { return a.second < b.second; });
        if(most == next.end() || most->second + 1 < names.size()) return prefix;
        prefix += most->first;
    }
}
} // namespace cg
