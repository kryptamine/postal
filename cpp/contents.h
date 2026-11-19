// What each element shows, gathered in one pass up the tree, for the passes
// before layout that ask it of every box.
#ifndef CG_CONTENTS_H
#define CG_CONTENTS_H

#include <litehtml.h>

#include <string>
#include <unordered_map>

namespace cg
{
// Words past this many cells are not kept: no button label is that long.
const int WORDS_KEPT = 40;

struct Holds
{
    // The words it shows, loosely squeezed; empty when `over`.
    std::string text;
    bool over = false;
    // The one block its words lie in, or `blocks` when more than one.
    const litehtml::element* block = nullptr;
    bool blocks = false;
    // A link inside it, or a filled box inside it, shows all its words.
    bool linked = false;
    bool filled = false;
    // An image inside it shows as a label.
    bool labelled = false;
    // Cells of all its words, however many.
    int length = 0;
    // A link, an `h1` with words, inside it.
    bool anchored = false;
    bool headed = false;

    std::string words() const;
    // Any text at all, a space even: whitespace between cells sizes a box.
    bool sized_by_text() const { return over || !text.empty(); }
    // Whitespace alone makes no line, so a box with only that stands empty.
    bool shows_nothing() const { return !over && !labelled && words().empty(); }
};

class Contents
{
  public:
    explicit Contents(const litehtml::element::ptr& root);
    const Holds& of(const litehtml::element* el) const;

  private:
    Holds gather(const litehtml::element::ptr& el);

    std::unordered_map<const litehtml::element*, Holds> holds;
};
} // namespace cg

#endif
