// Whether the mail says in words what an image's alt says.
#ifndef CG_SAID_ALREADY_H
#define CG_SAID_ALREADY_H

#include <litehtml.h>

#include <string>

namespace cg
{
// `alt` is in the text nearest `img`, or is all the words beside it say,
// or is a title `mail_text`, all the mail shows `folded`, has anywhere.
// Every comparison is of folded text, so case does not count.
bool said_already(const litehtml::element::ptr& img, const std::string& alt, const std::string& mail_text);
} // namespace cg

#endif
