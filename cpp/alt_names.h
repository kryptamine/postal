// What an image's alt names, apart from calling it a picture.
#ifndef CG_ALT_NAMES_H
#define CG_ALT_NAMES_H

#include <string>

namespace cg
{
// `alt` without the noun at its end that only calls it a picture:
// `Messaging icon` names `Messaging`, and `Some Image` names nothing.
std::string alt_name(const std::string& alt);
// Six words or more, a squeezed `alt` describes the picture rather than
// naming it: "An audience sits in a dark auditorium" says nothing a reader
// of the mail needs.
bool description(const std::string& alt);
// One word with a `_`, as `github_original_white`: the picture's file
// name, which names nothing a reader of the mail knows.
bool file_name(const std::string& alt);
// The words of a file's name, or only the first when `first`: an icon's
// file says what it shows, then how it is drawn.
std::string file_words(std::string alt, bool first);
// One lowercase word, as `dots` or `car`: the picture's file name, which
// says nothing beside the text the picture decorates.
bool file_word(const std::string& alt);
} // namespace cg

#endif
