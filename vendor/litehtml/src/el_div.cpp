#include "el_div.h"
#include "document.h"

litehtml::el_div::el_div(const document::ptr& doc) :
    html_tag(doc)
{
}

void litehtml::el_div::parse_attributes()
{
    map_align_attribute();
    html_tag::parse_attributes();
}
