// How wide the label of each image is: sized before layout, then settled
// against what the layout did with it, in the order the methods stand in.
// See docs/adr/0006-rendered-view-engine.md.
#ifndef CG_LABEL_WIDTHS_H
#define CG_LABEL_WIDTHS_H

#include <litehtml.h>

#include <vector>

namespace cg
{
class LabelWidths
{
  public:
    explicit LabelWidths(int cols = 0) : cols(cols) {}
    // Before layout: sizes `img` for a label `want` cells wide, one row
    // high, and widens the boxes sized for the picture. False when the
    // image has no room for any of it.
    bool fit(const litehtml::element::ptr& img, int want, bool icon, bool sole);
    // After a layout: a box of fixed width that a grown label spills past
    // loses that width. True when layout must run again.
    bool let_out_spilt();
    // After that layout: shrinks labels grown past their own width that
    // still spill, and their table's others. True when layout must run again.
    bool shrink_spilt();

  private:
    struct Grown
    {
        litehtml::element::ptr el;
        float own_px;
    };

    int cols;
    std::vector<Grown> grown;
};
} // namespace cg

#endif
