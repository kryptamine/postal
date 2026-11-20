// Digits the mail raises in `<sup>`: the cents of a price, a footnote's mark.
#ifndef CG_SUPERSCRIPTS_H
#define CG_SUPERSCRIPTS_H

#include <litehtml.h>

namespace cg
{
// Whether every word `el` shows is digits in a `<sup>`.
bool raised(const litehtml::element::ptr& el);
// Before layout: the digits in each `<sup>` under `root` show raised, as
// `58⁴⁶ €` rather than `5846 €`, a price a hundred times too high.
void raise_digits(const litehtml::element::ptr& root);
} // namespace cg

#endif
