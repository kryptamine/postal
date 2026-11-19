// What an image's alt names, apart from calling it a picture.
#ifndef CG_ALT_NAMES_H
#define CG_ALT_NAMES_H

#include <string>

namespace cg
{
// `alt` without the noun at its end that only calls it a picture:
// `Messaging icon` names `Messaging`, and `Some Image` names nothing.
std::string alt_name(const std::string& alt);
} // namespace cg

#endif
