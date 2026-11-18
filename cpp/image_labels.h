// Images are never fetched, so each one goes, or shows as a label one row
// high: its alt, or `[link]` for a link with nothing else to show. See
// docs/adr/0006-rendered-view-engine.md.
#ifndef CG_IMAGE_LABELS_H
#define CG_IMAGE_LABELS_H

#include "contents.h"

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
    // Lets each box holding pictures and no text shrink to them: they are a
    // label one row high now, or gone, and its own height would stand empty.
    // A spacer image's box keeps its gap.
    void fit_boxes(const Contents& contents);
    // After a layout: shrinks icon labels grown past their own width that
    // spill, and their table's others. True when layout must run again.
    bool shrink_spilt();
    // The label of an image `label` kept, by the src it gave it.
    const std::string* of(const std::string& src) const;

  private:
    struct Grown
    {
        litehtml::element::ptr el;
        float                  own_px;
    };

    void walk(const litehtml::element::ptr& el);
    void label_one(const litehtml::element::ptr& el);
    std::string text_for(const litehtml::element::ptr& el, bool icon, const litehtml::element::ptr& link,
                         bool sole);

    int                                 cols = 0;
    std::map<std::string, std::string>  labels;
    std::vector<Grown>                  grown;
    // All the text the mail shows, squeezed.
    std::string                         mail_text;
    std::set<const litehtml::element*>  redundant;
    std::vector<litehtml::element::ptr> pictures;
    // Links that already show `[link]` once.
    std::set<const litehtml::element*>  labelled_links;
};
} // namespace cg

#endif
