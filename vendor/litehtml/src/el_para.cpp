#include "el_para.h"
#include "document.h"

litehtml::el_para::el_para(const std::shared_ptr<document>& doc) :
    html_tag(doc)
{
}

void litehtml::el_para::parse_attributes()
{
    map_align_attribute();

    html_tag::parse_attributes();
}
