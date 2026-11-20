// A litehtml container that lays HTML mail out on the terminal cell grid.
// Every glyph is one cell of 8x16 CSS px, so CSS boxes land on cells; images
// are never fetched. See docs/adr/0006-rendered-view-engine.md.
#include "cell_grid.h"

#include "alignment.h"
#include "buttons.h"
#include "cell_gaps.h"
#include "colour.h"
#include "contents.h"
#include "dom.h"
#include "floated_cells.h"
#include "font_sizes.h"
#include "geometry.h"
#include "grid.h"
#include "image_labels.h"
#include "loosen.h"
#include "picture_boxes.h"
#include "snap.h"
#include "stacking.h"
#include "superscripts.h"
#include "text.h"
#include "titles.h"
#include "word_breaks.h"

#include <litehtml.h>

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <functional>
#include <memory>
#include <string>
#include <vector>

using namespace litehtml;
using namespace cg;

namespace
{
struct Font
{
    bool zero = false, bold = false, italic = false, underline = false;
    float size = 0;
};

// A corner rounded this much or more is drawn round, as `╭`.
const float ROUND_PX = 4;

bool round(pixel_t x, pixel_t y) { return float(x) >= ROUND_PX && float(y) >= ROUND_PX; }

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

struct Frame
{
    Box box;
    borders b;
    std::vector<position> clips;
};

class grid_container : public document_container, public Grid
{
  public:
    std::vector<position> clips;
    std::vector<Frame> frames;
    ImageLabels images;
    // A glyph fell past the last column, where the grid has no cell.
    bool lost_text = false;
    // Long words are parsed in pieces a line may break between.
    bool break_words;
    // The size the mail's body is set in (`body_size`).
    float body = 0;

    grid_container(int cols, bool break_words) : Grid(cols), break_words(break_words) {}

    void split_text(const char* text, const std::function<void(const char*)>& on_word,
                    const std::function<void(const char*)>& on_space) override
    {
        if(!break_words) return document_container::split_text(text, on_word, on_space);
        auto in_pieces = [&on_word](const char* word) {
            for(auto& piece : breakable(word)) on_word(piece.c_str());
        };
        document_container::split_text(text, in_pieces, on_space);
    }

    // Back to an empty grid, to draw a layout that changed.
    void wipe()
    {
        drawn.clear();
        frames.clear();
        clips.clear();
        lost_text = false;
    }

    bool visible(int row, int col, const position* extra = nullptr) const
    {
        float x = (float(col) + 0.5f) * CW, y = (float(row) + 0.5f) * CH;
        auto inside = [x, y](const position& p) {
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
        auto f = new Font;
        f->zero = float(d.size) < 4.f;
        f->bold = d.weight >= 600 || float(d.size) >= 20.f;
        f->italic = d.style == font_style_italic;
        f->underline = (d.decoration_line & text_decoration_line_underline) != 0;
        f->size = float(d.size);
        if(fm)
        {
            float h = f->zero ? 0 : CH;
            fm->font_size = d.size;
            fm->height = h;
            fm->ascent = h * 0.75f;
            fm->descent = h * 0.25f;
            fm->x_height = h * 0.5f;
            fm->ch_width = f->zero ? 0 : CW;
            fm->sub_shift = 0;
            fm->super_shift = 0;
        }
        return reinterpret_cast<uint_ptr>(f);
    }

    void delete_font(uint_ptr f) override { delete reinterpret_cast<Font*>(f); }

    pixel_t text_width(const char* text, uint_ptr font) override
    {
        return reinterpret_cast<Font*>(font)->zero ? 0 : float(cells(text)) * CW;
    }

    void put(int row, int col, const char* text, uint32_t fg, uint32_t flags, const position* clip = nullptr,
             int last = INT32_MAX)
    {
        for(const char* s = text; *s && col <= last;)
        {
            const char* start = s;
            uint32_t cp;
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
                c->fg = fg;
                c->flags = CG_TEXT | flags;
                if(w == 2)
                    if(cg_cell* tail = at(row, col + 1))
                    {
                        tail->flags = CG_TEXT | CG_WIDE_TAIL;
                        tail->bg = c->bg;
                    }
            }
            col += w;
        }
    }

    void draw_text(uint_ptr, const char* text, uint_ptr font, web_color color, const position& pos) override
    {
        auto f = reinterpret_cast<Font*>(font);
        if(f->zero || color.alpha == 0) return;
        uint32_t flags = (f->bold ? CG_BOLD : 0) | (f->italic ? CG_ITALIC : 0) | (f->underline ? CG_UNDERLINE : 0) |
                         size_flags(f->size, body);
        put(first_cell(pos.y, CH), first_cell(pos.x, CW), text, rgb(color), flags);
    }

    [[nodiscard]] pixel_t pt_to_px(float pt) const override { return pt * 96 / 72; }
    [[nodiscard]] pixel_t get_default_font_size() const override { return 16; }
    [[nodiscard]] const char* get_default_font_name() const override { return "monospace"; }

    void draw_list_marker(uint_ptr, const list_marker& m) override
    {
        const char* glyph = m.marker_type == list_style_type_circle   ? "◦"
                            : m.marker_type == list_style_type_square ? "▪"
                                                                      : "•";
        int row = int(std::floor((float(m.pos.y) + float(m.pos.height) / 2) / CH));
        put(row, first_cell(m.pos.x, CW), glyph, rgb(m.color), 0);
    }

    // Images are never fetched: each one left shows its label, in the box
    // `ImageLabels` sized for it.
    void load_image(const char*, const char*, bool) override {}

    void get_image_size(const char* src, const char*, size& sz) override
    {
        const std::string* label = images.of(src);
        sz.width = label ? float(cells(label->c_str())) * CW : 0;
        sz.height = label ? CH : 0;
    }

    void draw_image(uint_ptr, const background_layer& layer, const std::string& url, const std::string&) override
    {
        const std::string* label = images.of(url);
        if(!label) return;
        auto [r0, r1, c0, c1] = cells_of(layer.border_box);
        if(r1 < r0 || c1 < c0) return;
        std::string alt = fit(*label, c1 - c0 + 1);
        int len = cells(alt.c_str());
        put((r0 + r1) / 2, std::max(c0, (c0 + c1 + 1 - len) / 2), alt.c_str(), 0, CG_ALT, &layer.clip_box, c1);
    }

    void draw_solid_fill(uint_ptr, const background_layer& layer, const web_color& color) override
    {
        if(color.alpha == 0) return;
        auto [r0, r1, c0, c1] = cells_of(layer.border_box);
        if(layer.is_root)
        {
            c0 = 0;
            c1 = cols - 1;
        }

        // A button or a badge is short and narrow; a section is neither.
        uint32_t panel = layer.is_root || r1 - r0 >= 3 || (c1 - c0 + 1) * 2 >= cols ? CG_BG_PANEL : 0;
        const border_radiuses& radius = layer.border_radius;
        if(round(radius.top_left_x, radius.top_left_y) && round(radius.bottom_right_x, radius.bottom_right_y))
            panel |= CG_BG_ROUND;
        for(int r = r0; r <= r1; r++)
            for(int c = c0; c <= c1; c++)
                if(cg_cell* cell = at(r, c); cell && (layer.is_root || visible(r, c, &layer.clip_box)))
                    cell->bg =
                        (color.alpha == 255 ? rgb(color) : blend(cell->bg, rgb(color), float(color.alpha) / 255.f)) |
                        panel;
    }

    // A gradient is filled with its first colour.
    void draw_linear_gradient(uint_ptr hdc, const background_layer& l,
                              const background_layer::linear_gradient& g) override
    {
        if(!g.color_points.empty()) draw_solid_fill(hdc, l, g.color_points.front().color);
    }
    void draw_radial_gradient(uint_ptr hdc, const background_layer& l,
                              const background_layer::radial_gradient& g) override
    {
        if(!g.color_points.empty()) draw_solid_fill(hdc, l, g.color_points.front().color);
    }
    void draw_conic_gradient(uint_ptr hdc, const background_layer& l,
                             const background_layer::conic_gradient& g) override
    {
        if(!g.color_points.empty()) draw_solid_fill(hdc, l, g.color_points.front().color);
    }

    // Borders are drawn after the text, so they can step off a line of text
    // rather than cross it.
    void draw_borders(uint_ptr, const borders& b, const position& pos, bool) override
    {
        auto [r0, r1, c0, c1] = cells_of(pos);
        if(r1 < r0) r0 = r1 = int(std::floor(float(pos.y) / CH));
        if(c1 < c0) c0 = c1 = int(std::floor(float(pos.x) / CW));
        frames.push_back({{r0, r1, c0, c1}, b, clips});
    }

    void line(int row, int col, const char* glyph, const border& b, uint32_t flags, bool corner = false)
    {
        cg_cell* c = at(row, col);
        if(!c || ((c->flags & CG_TEXT) && !corner) || !visible(row, col)) return;
        if(corner && !is(*c, "─") && !is(*c, "│")) return;
        set_glyph(*c, glyph, std::strlen(glyph));
        c->fg = rgb(b.color);
        c->flags = flags;
    }

    [[nodiscard]] bool row_has_text(int row, int c0, int c1) const
    {
        if(row < 0 || row >= rows()) return false;
        for(int c = std::max(c0, 0); c <= std::min(c1, cols - 1); c++)
        {
            const cg_cell& cell = drawn[size_t(row) * size_t(cols) + size_t(c)];
            if((cell.flags & CG_TEXT) && !is(cell, " ")) return true;
        }
        return false;
    }

    // The cells every glyph of the mail's lies within, before any border is
    // drawn.
    [[nodiscard]] Box content_box() const
    {
        Box box{INT32_MAX, -1, INT32_MAX, -1};
        for(int i = 0; i < int(drawn.size()); i++)
            if(!is(drawn[size_t(i)], " "))
            {
                box.r0 = std::min(box.r0, i / cols), box.r1 = std::max(box.r1, i / cols);
                box.c0 = std::min(box.c0, i % cols), box.c1 = std::max(box.c1, i % cols);
            }
        return box;
    }

    // A horizontal border on a row of text moves one row out, or is dropped
    // when that row has text too.
    void apply_borders()
    {
        auto on = [](const border& e) {
            return float(e.width) > 0.f && e.style != border_style_none && e.style != border_style_hidden;
        };
        const Box mail = content_box();
        for(auto& f : frames)
        {
            clips = f.clips;
            auto [r0, r1, c0, c1] = f.box;
            bool top = on(f.b.top), bottom = on(f.b.bottom), left = on(f.b.left), right = on(f.b.right);
            bool page = top && bottom && left && right && f.box.holds(mail);
            uint32_t flags = page ? CG_BORDER | CG_PAGE_BOX : CG_BORDER;
            int tr = r0, br = r1;
            if(top && row_has_text(tr, c0 + left, c1 - right))
            {
                tr = r0 - 1;
                if(row_has_text(tr, c0, c1))
                {
                    top = false;
                    tr = r0;
                }
            }
            if(bottom && row_has_text(br, c0 + left, c1 - right))
            {
                br = r1 + 1;
                if(row_has_text(br, c0, c1))
                {
                    bottom = false;
                    br = r1;
                }
            }
            if(top)
                for(int c = c0; c <= c1; c++) line(tr, c, "─", f.b.top, flags);
            if(bottom)
                for(int c = c0; c <= c1; c++) line(br, c, "─", f.b.bottom, flags);
            if(left)
                for(int r = tr; r <= br; r++) line(r, c0, "│", f.b.left, flags);
            if(right)
                for(int r = tr; r <= br; r++) line(r, c1, "│", f.b.right, flags);
            const border_radiuses& rad = f.b.radius;
            if(top && left) line(tr, c0, round(rad.top_left_x, rad.top_left_y) ? "╭" : "┌", f.b.top, flags, true);
            if(top && right) line(tr, c1, round(rad.top_right_x, rad.top_right_y) ? "╮" : "┐", f.b.top, flags, true);
            if(bottom && left)
                line(br, c0, round(rad.bottom_left_x, rad.bottom_left_y) ? "╰" : "└", f.b.bottom, flags, true);
            if(bottom && right)
                line(br, c1, round(rad.bottom_right_x, rad.bottom_right_y) ? "╯" : "┘", f.b.bottom, flags, true);
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
        m.type = media_type_screen;
        m.width = float(cols) * CW;
        m.height = 1000 * CH;
        m.device_width = m.width;
        m.device_height = m.height;
        m.color = 8;
        m.monochrome = 0;
        m.color_index = 0;
        m.resolution = 96;
    }

    void transform_text(std::string& text, text_transform tt) override
    {
        for(auto& ch : text)
            if(tt == text_transform_uppercase) ch = char(toupper(static_cast<unsigned char>(ch)));
            else if(tt == text_transform_lowercase) ch = char(tolower(static_cast<unsigned char>(ch)));
    }

    element::ptr create_element(const char*, const string_map&, const document::ptr&) override { return nullptr; }
    void get_language(std::string& l, std::string& c) const override
    {
        l = "en";
        c = "";
    }
    void link(const document::ptr&, const element::ptr&) override {}
    void set_caption(const char*) override {}
    void set_base_url(const char*) override {}
    void on_anchor_click(const char*, const element::ptr&) override {}
    void on_mouse_event(const element::ptr&, mouse_event) override {}
    void set_cursor(const char*) override {}
    void import_css(std::string&, const std::string&, std::string&) override {}
};

// A border is a whole cell, and a box's own background under it would show
// round the line. litehtml leaves a table under `align=center` or `<center>`
// on the left.
const char* const user_css = "* { background-clip: padding-box !important; } "
                             "[align=center] > table, center > table { margin-left: auto; margin-right: auto; }";
} // namespace

struct cg_grid
{
    grid_container container;
    std::vector<std::string> hrefs;

    cg_grid(int cols, bool break_words) : container(cols, break_words) {}

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
                    auto [r0, r1, c0, c1] = cells_of(p);
                    for(int r = r0; r <= r1 && r < container.rows(); r++)
                        for(int c = c0; c <= c1; c++)
                            if(cg_cell* cell = container.at(r, c)) cell->link = id;
                }
                return;
            }
        for(auto& child : el->children()) mark_links(child);
    }
};

namespace
{
// The boxes were built from the styles before a pass changed them.
void lay_out(const document::ptr& doc, float width)
{
    doc->rebuild_render_tree();
    doc->render(width);
}

// Lays `doc` out as its styles now stand and draws it on an empty grid.
void draw(const document::ptr& doc, grid_container& container)
{
    float width = float(container.cols) * CW;
    LabelWidths& labels = container.images.widths();
    lay_out(doc, width);
    if(labels.let_out_spilt()) lay_out(doc, width);
    bool spilt = labels.shrink_spilt();
    if(left_align_paragraphs(doc->root()) || spilt) lay_out(doc, width);
    container.wipe();
    // Wider than the pane: litehtml skips a box wholly outside the clip, and
    // text there must be drawn to count as lost.
    position clip(0, 0, std::max(width, float(doc->width())), doc->height());
    doc->draw(0, 0, 0, &clip);
    container.apply_borders();
}

// `loose` is the second try at a mail that lost text past the pane: its
// long words break, its fixed widths go, and the tables round what still
// spills are stacked. See docs/adr/0006-rendered-view-engine.md.
cg_grid* rendered(const char* html, size_t len, int32_t cols, bool loose)
{
    auto grid = std::make_unique<cg_grid>(cols, loose);
    grid_container& container = grid->container;
    auto doc =
        document::createFromString(estring(std::string(html, len), encoding::utf_8), &container, master_css, user_css);
    if(!doc) return nullptr;
    container.images.label(doc->root(), cols);
    std::vector<element::ptr> buttons;
    {
        Contents contents(doc->root());
        fit_picture_boxes(container.images.pictures(), contents);
        buttons = flatten_buttons(doc->root(), contents);
        unframe_titles(doc->root(), contents);
    }
    left_align_spread(doc->root());
    unfloat_cells(doc->root());
    // Before the digits are raised: a cell of raised digits keeps no gap.
    gap_cells(doc->root());
    raise_digits(doc->root());
    snap_tree(doc->root());
    container.body = body_size(doc->root());
    if(loose) loosen(doc->root());
    draw(doc, container);
    if(loose && container.lost_text && stack_spilt_tables(doc->root(), float(cols) * CW)) draw(doc, container);
    shape_buttons(buttons, container);
    grid->mark_links(doc->root());
    keep_small_print(container);
    return grid.release();
}
} // namespace

extern "C" cg_grid* cg_render(const char* html, size_t len, int32_t cols)
{
    try
    {
        cg_grid* grid = rendered(html, len, cols, false);
        if(!grid || !grid->container.lost_text) return grid;
        delete grid;
        return rendered(html, len, cols, true);
    } catch(...)
    {
        return nullptr;
    }
}

extern "C" void cg_free(cg_grid* grid) { delete grid; }

extern "C" const cg_cell* cg_cells(const cg_grid* grid, size_t* rows)
{
    *rows = size_t(grid->container.rows());
    return grid->container.drawn.data();
}

extern "C" int32_t cg_fits(const cg_grid* grid) { return grid->container.lost_text ? 0 : 1; }

extern "C" const char* cg_href(const cg_grid* grid, uint32_t link)
{
    return link >= 1 && link <= grid->hrefs.size() ? grid->hrefs[link - 1].c_str() : nullptr;
}
