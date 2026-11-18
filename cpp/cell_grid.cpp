// A litehtml container that lays HTML mail out on the terminal cell grid.
// Every glyph is one cell of 8x16 CSS px, so CSS boxes land on cells; images
// are never fetched. See docs/adr/0006-rendered-view-engine.md.
#include "cell_grid.h"

#include <litehtml.h>

#include <algorithm>
#include <cmath>
#include <cstring>
#include <map>
#include <string>
#include <vector>

using namespace litehtml;

namespace
{
const float CW = 8, CH = 16;

uint32_t rgb(const web_color& c)
{
    return 0x01000000u | (uint32_t(c.red) << 16) | (uint32_t(c.green) << 8) | c.blue;
}

uint32_t blend(uint32_t under, uint32_t over, float a)
{
    // Mail colours assume a white page under them.
    if(!under) under = 0x01FFFFFF;
    auto mix = [a](uint32_t u, uint32_t o) {
        return uint32_t(std::lround(float(u) + (float(o) - float(u)) * a));
    };
    uint32_t r = mix((under >> 16) & 255, (over >> 16) & 255);
    uint32_t g = mix((under >> 8) & 255, (over >> 8) & 255);
    uint32_t b = mix(under & 255, over & 255);
    return 0x01000000u | (r << 16) | (g << 8) | b;
}

struct Font
{
    bool zero = false, bold = false, italic = false, underline = false;
};

void utf8_next(const char*& s, uint32_t& cp)
{
    auto u = reinterpret_cast<const unsigned char*>(s);
    if(u[0] < 0x80) { cp = u[0]; s += 1; }
    else if((u[0] >> 5) == 6 && u[1]) { cp = ((u[0] & 31u) << 6) | (u[1] & 63u); s += 2; }
    else if((u[0] >> 4) == 14 && u[1] && u[2]) { cp = ((u[0] & 15u) << 12) | ((u[1] & 63u) << 6) | (u[2] & 63u); s += 3; }
    else if(u[1] && u[2] && u[3]) { cp = ((u[0] & 7u) << 18) | ((u[1] & 63u) << 12) | ((u[2] & 63u) << 6) | (u[3] & 63u); s += 4; }
    else { cp = 0xFFFD; s += 1; }
}

int cp_width(uint32_t cp)
{
    if((cp >= 0x300 && cp <= 0x36F) || (cp >= 0x200B && cp <= 0x200F) || (cp >= 0xFE00 && cp <= 0xFE0F) ||
       cp == 0xFEFF || cp == 0x34F || cp == 0xAD || (cp >= 0xE0000 && cp <= 0xE007F))
        return 0;
    if((cp >= 0x1100 && cp <= 0x115F) || (cp >= 0x2E80 && cp <= 0xA4CF) || (cp >= 0xAC00 && cp <= 0xD7A3) ||
       (cp >= 0xF900 && cp <= 0xFAFF) || (cp >= 0xFE30 && cp <= 0xFE4F) || (cp >= 0xFF00 && cp <= 0xFF60) ||
       (cp >= 0xFFE0 && cp <= 0xFFE6) || (cp >= 0x1F300 && cp <= 0x1F64F) || (cp >= 0x1F680 && cp <= 0x1F6FF) ||
       (cp >= 0x1F900 && cp <= 0x1FAFF) || (cp >= 0x20000 && cp <= 0x3FFFD))
        return 2;
    return 1;
}

// The cells whose centre lies in [a, a + len). Text rows use it too, so a
// line and the box around it round the same way.
void span(float a, float len, float unit, int& first, int& last)
{
    first = int(std::ceil(a / unit - 0.5f));
    last  = int(std::ceil((a + len) / unit - 0.5f)) - 1;
}

int first_cell(float a, float unit) { return int(std::ceil(a / unit - 0.5f)); }

void set_glyph(cg_cell& c, const char* g, size_t n)
{
    n = std::min(n, sizeof c.glyph - 1);
    std::memcpy(c.glyph, g, n);
    c.glyph[n] = 0;
}

void append_glyph(cg_cell& c, const char* g, size_t n)
{
    size_t len = std::strlen(c.glyph);
    if(len + n < sizeof c.glyph)
    {
        std::memcpy(c.glyph + len, g, n);
        c.glyph[len + n] = 0;
    }
}

bool is(const cg_cell& c, const char* g) { return std::strcmp(c.glyph, g) == 0; }

bool shown(const css_border& b)
{
    return b.width.val() > 0 && b.style != border_style_none && b.style != border_style_hidden;
}

// Whole rows. Half a row rounds up above a box and down below it, so the
// 8px under one paragraph and the 8px over the next make one row together.
float rows_above(float px)
{
    return std::copysign(std::floor(std::fabs(px) / CH + 0.5f) * CH, px);
}

float rows_below(float px)
{
    return std::copysign(std::ceil(std::fabs(px) / CH - 0.5f) * CH, px);
}

float columns(float px)
{
    float n = std::floor(std::fabs(px) / CW + 0.5f);
    return std::copysign(n * CW, px);
}

void snap(css_length& l, float (*to)(float))
{
    if(!l.is_predefined() && l.units() == css_units_px) l.set_value(to(l.val()), css_units_px);
}

// Box edges snap to whole cells before layout, so a separator gets a row of
// its own and a button's label sits in the middle of its block. A shown
// border is one cell thick: a row above and below, a column at each side.
void snap_tree(const element::ptr& el)
{
    css_properties& css = el->css();
    css_margins     m   = css.get_margins();
    snap(m.top, rows_above);
    snap(m.bottom, rows_below);
    snap(m.left, columns);
    snap(m.right, columns);
    css.set_margins(m);

    css_margins p = css.get_padding();
    snap(p.top, rows_above);
    snap(p.bottom, rows_below);
    snap(p.left, columns);
    snap(p.right, columns);
    css.set_padding(p);

    css_borders b = css.get_borders();
    if(shown(b.top)) b.top.width = CH;
    if(shown(b.bottom)) b.bottom.width = CH;
    if(shown(b.left)) b.left.width = CW;
    if(shown(b.right)) b.right.width = CW;
    css.set_borders(b);

    css_length h = css.get_height();
    snap(h, rows_above);
    css.set_height(h);
    css_length min_h = css.get_min_height();
    snap(min_h, rows_above);
    css.set_min_height(min_h);
    // An image shown as its alt is one row high, whatever width scales it.
    if(std::strcmp(el->get_tagName(), "img") == 0 && h.is_predefined())
        if(const char* alt = el->get_attr("alt"); alt && *alt) css.set_height(css_length(CH, css_units_px));

    css_length spacing = css.get_border_spacing_y();
    snap(spacing, rows_above);
    css.set_border_spacing_y(spacing);
    // A collapsed border gives each cell half its width, which puts a rule
    // between two rows; separate borders with no spacing look the same.
    if(css.get_border_collapse() == border_collapse_collapse)
    {
        css.set_border_collapse(border_collapse_separate);
        css.set_border_spacing_x(css_length(0, css_units_px));
        css.set_border_spacing_y(css_length(0, css_units_px));
    }

    for(auto& child : el->children()) snap_tree(child);
}

struct Frame
{
    int                   r0, r1, c0, c1;
    borders               b;
    std::vector<position> clips;
};

class grid_container : public document_container
{
  public:
    int                                cols;
    std::vector<cg_cell>               grid;
    std::vector<position>              clips;
    std::vector<Frame>                 frames;
    std::map<std::string, std::string> alts;
    Font                               plain_font;
    // A glyph fell past the last column, where the grid has no cell.
    bool                               lost_text = false;

    explicit grid_container(int cols) : cols(cols) {}

    int rows() const { return cols ? int(grid.size()) / cols : 0; }

    cg_cell* at(int row, int col)
    {
        if(row < 0 || col < 0 || col >= cols) return nullptr;
        if(row >= rows())
        {
            cg_cell blank{};
            blank.glyph[0] = ' ';
            grid.resize(size_t(row + 1) * size_t(cols), blank);
        }

        return &grid[size_t(row) * size_t(cols) + size_t(col)];
    }

    bool visible(int row, int col, const position* extra = nullptr) const
    {
        float x = (float(col) + 0.5f) * CW, y = (float(row) + 0.5f) * CH;
        auto  inside = [x, y](const position& p) {
            float px = p.x, py = p.y, pw = p.width, ph = p.height;
            return x >= px && x < px + pw && y >= py && y < py + ph;
        };
        for(auto& c : clips)
            if(!inside(c)) return false;
        return !extra || inside(*extra);
    }

    // A font under 4px takes no room, which keeps the `font-size:0` trick of
    // MJML and Outlook layouts working. Large text is bold instead of larger.
    uint_ptr create_font(const font_description& d, const document*, font_metrics* fm) override
    {
        auto f       = new Font;
        f->zero      = float(d.size) < 4.f;
        f->bold      = d.weight >= 600 || float(d.size) >= 20.f;
        f->italic    = d.style == font_style_italic;
        f->underline = (d.decoration_line & text_decoration_line_underline) != 0;
        if(fm)
        {
            float h         = f->zero ? 0 : CH;
            fm->font_size   = d.size;
            fm->height      = h;
            fm->ascent      = h * 0.75f;
            fm->descent     = h * 0.25f;
            fm->x_height    = h * 0.5f;
            fm->ch_width    = f->zero ? 0 : CW;
            fm->sub_shift   = 0;
            fm->super_shift = 0;
        }
        return reinterpret_cast<uint_ptr>(f);
    }

    void delete_font(uint_ptr f) override { delete reinterpret_cast<Font*>(f); }

    pixel_t text_width(const char* text, uint_ptr font) override
    {
        if(reinterpret_cast<Font*>(font)->zero) return 0;
        int w = 0;
        for(const char* s = text; *s;)
        {
            uint32_t cp;
            utf8_next(s, cp);
            w += cp_width(cp);
        }
        return float(w) * CW;
    }

    void put(int row, int col, const char* text, uint32_t fg, uint32_t flags, const position* clip = nullptr,
             int last = INT32_MAX)
    {
        for(const char* s = text; *s && col <= last;)
        {
            const char* start = s;
            uint32_t    cp;
            utf8_next(s, cp);
            int w = cp_width(cp);
            if(w == 0)
            {
                if(cg_cell* prev = at(row, col - 1); prev && (prev->flags & CG_TEXT))
                    append_glyph(*prev, start, size_t(s - start));
                continue;
            }

            cg_cell* c = at(row, col);
            if(!c && col >= cols && cp != ' ' && cp != 0xA0) lost_text = true;
            if(c && visible(row, col, clip))
            {
                if(cp == 0xA0 || cp == 0x2007) set_glyph(*c, " ", 1);
                else set_glyph(*c, start, size_t(s - start));
                c->fg    = fg;
                c->flags = CG_TEXT | flags;
                if(w == 2)
                    if(cg_cell* tail = at(row, col + 1))
                    {
                        tail->flags = CG_TEXT | CG_WIDE_TAIL;
                        tail->bg    = c->bg;
                    }
            }
            col += w;
        }
    }

    void draw_text(uint_ptr, const char* text, uint_ptr font, web_color color, const position& pos) override
    {
        auto f = reinterpret_cast<Font*>(font);
        if(f->zero || color.alpha == 0) return;
        uint32_t flags = (f->bold ? CG_BOLD : 0) | (f->italic ? CG_ITALIC : 0) | (f->underline ? CG_UNDERLINE : 0);
        put(first_cell(pos.y, CH), first_cell(pos.x, CW), text, rgb(color), flags);
    }

    pixel_t     pt_to_px(float pt) const override { return pt * 96 / 72; }
    pixel_t     get_default_font_size() const override { return 16; }
    const char* get_default_font_name() const override { return "monospace"; }

    void draw_list_marker(uint_ptr, const list_marker& m) override
    {
        const char* glyph = m.marker_type == list_style_type_circle   ? "◦"
                            : m.marker_type == list_style_type_square ? "▪"
                                                                      : "•";
        int row = int(std::floor((float(m.pos.y) + float(m.pos.height) / 2) / CH));
        put(row, first_cell(m.pos.x, CW), glyph, rgb(m.color), 0);
    }

    // Images are never fetched. One with alt text claims a row, so its alt
    // shows; one without keeps only the box its own size gives.
    void load_image(const char*, const char*, bool) override {}

    void get_image_size(const char* src, const char*, size& sz) override
    {
        sz.width = sz.height = 0;
        auto it = alts.find(src);
        if(it == alts.end()) return;
        sz.width  = text_width(it->second.c_str(), reinterpret_cast<uint_ptr>(&plain_font));
        sz.height = CH;
    }

    void draw_image(uint_ptr, const background_layer& layer, const std::string& url, const std::string&) override
    {
        auto it = alts.find(url);
        if(it == alts.end()) return;
        int r0, r1, c0, c1;
        span(layer.border_box.y, layer.border_box.height, CH, r0, r1);
        span(layer.border_box.x, layer.border_box.width, CW, c0, c1);
        if(r1 < r0 || c1 < c0) return;
        const std::string& alt = it->second;
        int len = int(text_width(alt.c_str(), reinterpret_cast<uint_ptr>(&plain_font)) / CW);
        put((r0 + r1) / 2, std::max(c0, (c0 + c1 + 1 - len) / 2), alt.c_str(), 0, CG_ALT, &layer.clip_box, c1);
    }

    void draw_solid_fill(uint_ptr, const background_layer& layer, const web_color& color) override
    {
        if(color.alpha == 0) return;
        int r0, r1, c0, c1;
        span(layer.border_box.y, layer.border_box.height, CH, r0, r1);
        span(layer.border_box.x, layer.border_box.width, CW, c0, c1);
        if(layer.is_root) { c0 = 0; c1 = cols - 1; }
        for(int r = r0; r <= r1; r++)
            for(int c = c0; c <= c1; c++)
                if(cg_cell* cell = at(r, c); cell && (layer.is_root || visible(r, c, &layer.clip_box)))
                    cell->bg = color.alpha == 255 ? rgb(color) : blend(cell->bg, rgb(color), float(color.alpha) / 255.f);
    }

    // A gradient is filled with its first colour.
    void draw_linear_gradient(uint_ptr hdc, const background_layer& l, const background_layer::linear_gradient& g) override
    {
        if(!g.color_points.empty()) draw_solid_fill(hdc, l, g.color_points.front().color);
    }
    void draw_radial_gradient(uint_ptr hdc, const background_layer& l, const background_layer::radial_gradient& g) override
    {
        if(!g.color_points.empty()) draw_solid_fill(hdc, l, g.color_points.front().color);
    }
    void draw_conic_gradient(uint_ptr hdc, const background_layer& l, const background_layer::conic_gradient& g) override
    {
        if(!g.color_points.empty()) draw_solid_fill(hdc, l, g.color_points.front().color);
    }

    // Borders are drawn after the text, so they can step off a line of text
    // rather than cross it.
    void draw_borders(uint_ptr, const borders& b, const position& pos, bool) override
    {
        int r0, r1, c0, c1;
        span(pos.y, pos.height, CH, r0, r1);
        span(pos.x, pos.width, CW, c0, c1);
        if(r1 < r0) r0 = r1 = int(std::floor(float(pos.y) / CH));
        if(c1 < c0) c0 = c1 = int(std::floor(float(pos.x) / CW));
        frames.push_back({r0, r1, c0, c1, b, clips});
    }

    void line(int row, int col, const char* glyph, const border& b, bool corner = false)
    {
        cg_cell* c = at(row, col);
        if(!c || ((c->flags & CG_TEXT) && !corner) || !visible(row, col)) return;
        if(corner && !is(*c, "─") && !is(*c, "│")) return;
        set_glyph(*c, glyph, std::strlen(glyph));
        c->fg    = rgb(b.color);
        c->flags = CG_BORDER;
    }

    bool row_has_text(int row, int c0, int c1) const
    {
        if(row < 0 || row >= rows()) return false;
        for(int c = std::max(c0, 0); c <= std::min(c1, cols - 1); c++)
        {
            const cg_cell& cell = grid[size_t(row) * size_t(cols) + size_t(c)];
            if((cell.flags & CG_TEXT) && !is(cell, " ")) return true;
        }
        return false;
    }

    // A horizontal border on a row of text moves one row out, or is dropped
    // when that row has text too.
    void apply_borders()
    {
        auto on = [](const border& e) {
            return float(e.width) > 0.f && e.style != border_style_none && e.style != border_style_hidden;
        };
        for(auto& f : frames)
        {
            clips    = f.clips;
            bool top = on(f.b.top), bottom = on(f.b.bottom), left = on(f.b.left), right = on(f.b.right);
            int  tr = f.r0, br = f.r1;
            if(top && row_has_text(tr, f.c0 + left, f.c1 - right))
            {
                tr = f.r0 - 1;
                if(row_has_text(tr, f.c0, f.c1)) { top = false; tr = f.r0; }
            }
            if(bottom && row_has_text(br, f.c0 + left, f.c1 - right))
            {
                br = f.r1 + 1;
                if(row_has_text(br, f.c0, f.c1)) { bottom = false; br = f.r1; }
            }
            if(top)
                for(int c = f.c0; c <= f.c1; c++) line(tr, c, "─", f.b.top);
            if(bottom)
                for(int c = f.c0; c <= f.c1; c++) line(br, c, "─", f.b.bottom);
            if(left)
                for(int r = tr; r <= br; r++) line(r, f.c0, "│", f.b.left);
            if(right)
                for(int r = tr; r <= br; r++) line(r, f.c1, "│", f.b.right);
            if(top && left) line(tr, f.c0, "┌", f.b.top, true);
            if(top && right) line(tr, f.c1, "┐", f.b.top, true);
            if(bottom && left) line(br, f.c0, "└", f.b.bottom, true);
            if(bottom && right) line(br, f.c1, "┘", f.b.bottom, true);
        }
        clips.clear();
    }

    void set_clip(const position& pos, const border_radiuses&) override { clips.push_back(pos); }
    void del_clip() override
    {
        if(!clips.empty()) clips.pop_back();
    }

    void get_viewport(position& vp) const override { vp = position(0, 0, float(cols) * CW, 1000 * CH); }

    void get_media_features(media_features& m) const override
    {
        m.type          = media_type_screen;
        m.width         = float(cols) * CW;
        m.height        = 1000 * CH;
        m.device_width  = m.width;
        m.device_height = m.height;
        m.color         = 8;
        m.monochrome    = 0;
        m.color_index   = 0;
        m.resolution    = 96;
    }

    void transform_text(std::string& text, text_transform tt) override
    {
        for(auto& ch : text)
            if(tt == text_transform_uppercase) ch = char(toupper(static_cast<unsigned char>(ch)));
            else if(tt == text_transform_lowercase) ch = char(tolower(static_cast<unsigned char>(ch)));
    }

    element::ptr create_element(const char*, const string_map&, const document::ptr&) override { return nullptr; }
    void         get_language(std::string& l, std::string& c) const override { l = "en"; c = ""; }
    void         link(const document::ptr&, const element::ptr&) override {}
    void         set_caption(const char*) override {}
    void         set_base_url(const char*) override {}
    void         on_anchor_click(const char*, const element::ptr&) override {}
    void         on_mouse_event(const element::ptr&, mouse_event) override {}
    void         set_cursor(const char*) override {}
    void         import_css(std::string&, const std::string&, std::string&) override {}
};

// One line of text is one row, so line-height can only be one row. litehtml
// leaves a table under `align=center` or `<center>` on the left.
const char* const user_css = "* { line-height: normal !important; } "
                             "[align=center] > table, center > table { margin-left: auto; margin-right: auto; }";

void collect_alts(const element::ptr& el, std::map<std::string, std::string>& alts)
{
    if(std::strcmp(el->get_tagName(), "img") == 0)
    {
        const char* src = el->get_attr("src");
        const char* alt = el->get_attr("alt");
        if(src && alt && *alt) alts.emplace(src, alt);
    }
    for(auto& child : el->children()) collect_alts(child, alts);
}

void leaf_boxes(const element::ptr& el, std::vector<position>& out)
{
    if(el->children().empty())
    {
        if(el->get_render_item())
        {
            position p = el->get_placement();
            if(float(p.width) > 0 && float(p.height) > 0) out.push_back(p);
        }
        return;
    }
    for(auto& child : el->children()) leaf_boxes(child, out);
}
} // namespace

struct cg_grid
{
    grid_container           container;
    std::vector<std::string> hrefs;

    explicit cg_grid(int cols) : container(cols) {}

    // Each link's id goes on the cells of its leaf boxes: its words, or the
    // box of an image inside it.
    void mark_links(const element::ptr& el)
    {
        if(std::strcmp(el->get_tagName(), "a") == 0)
            if(const char* href = el->get_attr("href"); href && *href)
            {
                std::vector<position> boxes;
                leaf_boxes(el, boxes);
                if(boxes.empty()) return;
                hrefs.emplace_back(href);
                auto id = uint32_t(hrefs.size());
                for(auto& p : boxes)
                {
                    int r0, r1, c0, c1;
                    span(p.y, p.height, CH, r0, r1);
                    span(p.x, p.width, CW, c0, c1);
                    for(int r = r0; r <= r1 && r < container.rows(); r++)
                        for(int c = c0; c <= c1; c++)
                            if(cg_cell* cell = container.at(r, c)) cell->link = id;
                }
                return;
            }
        for(auto& child : el->children()) mark_links(child);
    }
};

extern "C" cg_grid* cg_render(const char* html, size_t len, int32_t cols)
{
    cg_grid* grid = nullptr;
    try
    {
        grid = new cg_grid(cols);
        auto doc = document::createFromString(estring(std::string(html, len), encoding::utf_8), &grid->container,
                                              master_css, user_css);
        if(!doc)
        {
            delete grid;
            return nullptr;
        }
        collect_alts(doc->root(), grid->container.alts);
        snap_tree(doc->root());
        // The boxes were built from the styles before the snap.
        doc->rebuild_render_tree();
        doc->render(float(cols) * CW);
        position clip(0, 0, float(cols) * CW, doc->height());
        doc->draw(0, 0, 0, &clip);
        grid->container.apply_borders();
        grid->mark_links(doc->root());
        return grid;
    } catch(...)
    {
        delete grid;
        return nullptr;
    }
}

extern "C" void cg_free(cg_grid* grid) { delete grid; }

extern "C" const cg_cell* cg_cells(const cg_grid* grid, size_t* rows)
{
    *rows = size_t(grid->container.rows());
    return grid->container.grid.data();
}

extern "C" int32_t cg_fits(const cg_grid* grid) { return grid->container.lost_text ? 0 : 1; }

extern "C" const char* cg_href(const cg_grid* grid, uint32_t link)
{
    return link >= 1 && link <= grid->hrefs.size() ? grid->hrefs[link - 1].c_str() : nullptr;
}
