// Where a word too long for a narrow pane may break.
#ifndef CG_WORD_BREAKS_H
#define CG_WORD_BREAKS_H

#include <string>
#include <vector>

namespace cg
{
// `word` in pieces a line may break between: after a mark of an address
// (`/`, `?`, `&`, `-`...) once a piece is 8 cells, and anywhere at 24. A
// word of at most 16 cells is one piece.
std::vector<std::string> breakable(const std::string& word);
} // namespace cg

#endif
