// What a link with nothing to show is called: the words of its page's
// address, or the site it opens.
#ifndef CG_LINK_NAMES_H
#define CG_LINK_NAMES_H

#include <string>

namespace cg
{
// The last path segment made of words (`watch-party-tickets-2004` reads
// `watch party tickets`), or else the host's domain without its subdomain:
// a tracking redirect's path names nothing. `link` for no http(s) address.
std::string link_name(const char* href);
} // namespace cg

#endif
