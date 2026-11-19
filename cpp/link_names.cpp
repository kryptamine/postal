#include "link_names.h"

#include <algorithm>
#include <cctype>
#include <vector>

namespace cg
{
namespace
{
std::vector<std::string> split(const std::string& s, const char* by)
{
    std::vector<std::string> parts;
    size_t start = 0;
    while(start <= s.size())
    {
        size_t end = s.find_first_of(by, start);
        if(end == std::string::npos) end = s.size();
        parts.push_back(s.substr(start, end - start));
        start = end + 1;
    }
    return parts;
}

bool all_of(const std::string& s, int (*is)(int))
{
    return !s.empty() && std::all_of(s.begin(), s.end(), [is](unsigned char c) { return is(c) != 0; });
}

// Lowercase words joined by `-` or `_`; a trailing number is an id. A
// tracking token has capitals or digits in its parts, so it is no slug.
std::string slug_words(std::string segment)
{
    segment = segment.substr(0, segment.find('.'));
    std::vector<std::string> words = split(segment, "-_");
    while(!words.empty() && all_of(words.back(), isdigit)) words.pop_back();
    if(words.size() < 2) return "";
    std::string out;
    int singles = 0;
    for(auto& w : words)
    {
        if(!all_of(w, islower)) return "";
        singles += w.size() == 1;
        out += (out.empty() ? "" : " ") + w;
    }
    // A token cut into letters is no words: `i-l-bhithz-gkkhdiuyd-y`.
    return singles <= 1 ? out : "";
}

bool second_level(const std::string& label)
{
    for(const char* l : {"ac", "co", "com", "edu", "gov", "net", "org"})
        if(label == l) return true;
    return false;
}

// The last two labels, or three under a country's second level (`co.uk`).
std::string domain(const std::string& host)
{
    std::vector<std::string> labels = split(host, ".");
    size_t keep = labels.size() >= 3 && labels.back().size() == 2 && second_level(labels[labels.size() - 2]) ? 3 : 2;
    if(labels.size() <= keep) return host;
    std::string out;
    for(size_t i = labels.size() - keep; i < labels.size(); i++) out += (out.empty() ? "" : ".") + labels[i];
    return out;
}
} // namespace

std::string link_name(const char* href)
{
    std::string url(href ? href : "");
    size_t scheme = url.find("://");
    if(scheme == std::string::npos || (url.compare(0, scheme, "http") != 0 && url.compare(0, scheme, "https") != 0))
        return "link";
    std::string rest = url.substr(scheme + 3);
    rest = rest.substr(0, rest.find_first_of("?#"));
    size_t slash = rest.find('/');
    std::string host = rest.substr(0, slash);
    host = host.substr(host.find('@') == std::string::npos ? 0 : host.find('@') + 1);
    host = host.substr(0, host.find(':'));
    std::transform(host.begin(), host.end(), host.begin(), [](unsigned char c) { return char(std::tolower(c)); });
    if(host.empty()) return "link";

    if(slash != std::string::npos)
    {
        std::vector<std::string> segments = split(rest.substr(slash + 1), "/");
        for(auto it = segments.rbegin(); it != segments.rend(); ++it)
            if(std::string words = slug_words(*it); !words.empty()) return words;
    }
    return domain(host);
}
} // namespace cg
