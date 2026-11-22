// What an image's alt names, and what of that a reader is shown.
#ifndef CG_ALT_NAMES_H
#define CG_ALT_NAMES_H

#include <string>

namespace cg
{
// `alt` without the noun at its end that only calls it a picture:
// `Messaging icon` names `Messaging`, and `Some Image` names nothing.
std::string alt_name(const std::string& alt);
// What of `alt`, an alt_name, the mail shows; `sole` when the picture is
// all its link shows. A file's name or a description names nothing a
// reader knows, and a file's name only says what a link's own picture opens.
std::string shown_alt(const std::string& alt, bool sole, bool icon);
} // namespace cg

#endif
