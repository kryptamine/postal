// Images are never fetched, so each one goes, or shows as a label one row
// high: its alt, or the name of a link with nothing else to show. See
// docs/adr/0006-rendered-view-engine.md.
#ifndef CG_IMAGE_LABELS_H
#define CG_IMAGE_LABELS_H

#include "label_widths.h"

#include <litehtml.h>

#include <map>
#include <set>
#include <string>
#include <vector>

namespace cg
{
class ImageLabels
{
  public:
    // Hides or sizes every image under `root` before layout, `cols` wide.
    void label(const litehtml::element::ptr& root, int cols);
    // The widths of the labels kept, to settle after a layout.
    LabelWidths& widths() { return label_widths; }
    // Images that say or open something, hidden or shown.
    [[nodiscard]] const std::vector<litehtml::element::ptr>& pictures() const { return pictured; }
    // The label of an image `label` kept, by the src it gave it.
    [[nodiscard]] const std::string* of(const std::string& src) const;

  private:
    void walk(const litehtml::element::ptr& el);
    void label_one(const litehtml::element::ptr& el);
    std::string text_for(const litehtml::element::ptr& el, bool icon, const litehtml::element::ptr& link, bool sole);
    [[nodiscard]] std::string named(const litehtml::element::ptr& img, bool icon) const;

    std::map<std::string, std::string> labels;
    LabelWidths label_widths;
    // All the text the mail shows, squeezed.
    std::string mail_text;
    std::set<const litehtml::element*> redundant;
    std::vector<litehtml::element::ptr> pictured;
    // The words the icons of a table all start their names with.
    std::map<const litehtml::element*, std::string> prefixes;
    // Links that already show their name once.
    std::set<const litehtml::element*> labelled_links;
};
} // namespace cg

#endif
