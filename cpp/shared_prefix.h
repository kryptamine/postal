// The words a row of icons all start their names with.
#ifndef CG_SHARED_PREFIX_H
#define CG_SHARED_PREFIX_H

#include <string>
#include <vector>

namespace cg
{
// The leading words all of `names` but one at most share, each with more
// after them; empty under three names. `Kraken on X` then names `X`.
std::string shared_prefix(const std::vector<std::string>& names);
} // namespace cg

#endif
