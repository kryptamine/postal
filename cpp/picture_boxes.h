// The boxes round pictures, once each picture is a label one row high or
// gone. See docs/adr/0006-rendered-view-engine.md.
#ifndef CG_PICTURE_BOXES_H
#define CG_PICTURE_BOXES_H

#include "contents.h"

#include <litehtml.h>

#include <vector>

namespace cg
{
// A box with pictures and no text shrinks to them, and one a gone picture
// left showing nothing goes, as does a table column of such cells. A spacer
// image is no picture, and its box keeps its gap.
void fit_picture_boxes(const std::vector<litehtml::element::ptr>& pictures, const Contents& contents);
} // namespace cg

#endif
