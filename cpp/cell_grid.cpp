// A litehtml container that lays HTML mail out on the terminal cell grid.
// Every glyph is one cell of 8x16 CSS px, so CSS boxes land on cells; images
// are never fetched. See docs/adr/0006-rendered-view-engine.md.
#include "cell_grid.h"

#include "alignment.h"
#include "borders.h"
#include "buttons.h"
#include "cell_gaps.h"
#include "colour.h"
#include "contents.h"
#include "dom.h"
#include "floated_cells.h"
#include "font_sizes.h"
#include "geometry.h"
#include "grid.h"
#include "hidden.h"
#include "image_labels.h"
#include "loosen.h"
#include "picture_boxes.h"
#include "regrouped_cells.h"
#include "row_heights.h"
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
    bool zero = false, bold = false, italic = false, underline = false, strike = false;
    float size = 0;
};

// A corner rounded this much or more is drawn round, as `╭`.
const float ROUND_PX = 4;
// The screen width media queries get for a mail's desktop layout.
const float DESKTOP_PX = 1024;
// Mail sets its mobile breakpoint at 600 to 640 px: a wider pane gets the
// desktop layout already.
const float PHONE_PX = 640;

// The ways a mail is laid out, tried in turn. See
// docs/adr/0006-rendered-view-engine.md.
enum class Try : uint8_t
{
    // As the pane is wide.
    pane,
    // Again after text spilt past the pane: long words break, fixed widths go.
    loose,
    // Media queries answered as on a desktop screen, for a layout set for a phone.
    desktop,
};

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

void clear_cell(cg_cell& c)
{
    set_glyph(c, " ", 1);
    c.fg = 0;
    c.flags = 0;
}

class grid_container : public document_container, public Grid
{
  public:
    std::vector<position> clips;
    std::vector<Frame> frames;
    ImageLabels images;
    // A glyph fell past the last column, where the grid has no cell.
    bool lost_text = false;
    // A side of a box was past the pane, where its words were not.
    bool lost_border = false;
    Try how;
    // The size the mail's body is set in (`body_size`).
    float body = 0;

    grid_container(int cols, Try how) : Grid(cols), how(how) {}

    // Something drawn fell past the pane. Only lost text makes a layout
    // unfit to show; a lost border is still worth a looser try.
    [[nodiscard]] bool spilt() const { return lost_text || lost_border; }

    void split_text(const char* text, const std::function<void(const char*)>& on_word,
                    const std::function<void(const char*)>& on_space) override
    {
        // Long words are parsed in pieces a line may break between.
        if(how != Try::loose) return document_container::split_text(text, on_word, on_space);
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
        lost_border = false;
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
        f->strike = (d.decoration_line & text_decoration_line_line_through) != 0;
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

    void put(int row, int col, const char* text, uint32_t fg, uint8_t alpha, uint32_t flags,
             const position* clip = nullptr, int last = INT32_MAX)
    {
        for(const char* s = text; *s && col <= last;)
        {
            const char* start = s;
            uint32_t cp = 0;
            int w = 0;
            next_cluster(s, cp, w);
            if(w == 0)
            {
                if(cg_cell* prev = at(row, col - 1); prev && (prev->flags & CG_TEXT))
                    append_glyph(*prev, start, size_t(s - start));
                continue;
            }

            if(w == 2 && col + 1 >= cols)
            {
                if(col >= cols - 1) lost_text = true;
                col += w;
                continue;
            }

            cg_cell* c = at(row, col);
            if(!c && col >= cols && cp != ' ' && cp != 0xA0) lost_text = true;
            bool blank = cp == ' ' || cp == 0xA0 || cp == 0x2007;
            // A space between two floats is laid over the first one's words.
            bool over_word = blank && c && (c->flags & CG_TEXT) && !is(*c, " ");
            if(c && visible(row, col, clip) && !over_word)
            {
                if(c->flags & CG_WIDE_TAIL)
                    if(cg_cell* left = at(row, col - 1)) clear_cell(*left);
                if(cg_cell* after = at(row, col + w); after && (after->flags & CG_WIDE_TAIL)) clear_cell(*after);
                if(cp == 0xA0 || cp == 0x2007) set_glyph(*c, " ", 1);
                else set_glyph(*c, start, size_t(s - start));
                c->fg = alpha == 255 ? fg : blend(c->bg, fg, float(alpha) / 255.f);
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
                         (f->strike ? CG_STRIKE : 0) | size_flags(f->size, body);
        put(first_cell(pos.y, CH), first_cell(pos.x, CW), text, rgb(color), color.alpha, flags);
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
        put(row, first_cell(m.pos.x, CW), glyph, rgb(m.color), m.color.alpha, 0);
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
        put((r0 + r1) / 2, std::max(c0, (c0 + c1 + 1 - len) / 2), alt.c_str(), 0, 255, CG_ALT, &layer.clip_box, c1);
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
        uint32_t colour = rgb(color);
        for(int r = r0; r <= r1; r++)
            for(int c = c0; c <= c1; c++)
                if(cg_cell* cell = at(r, c); cell && (layer.is_root || visible(r, c, &layer.clip_box)))
                {
                    uint32_t keep_round = (cell->bg & CG_BG_ROUND) && alike(colour, cell->bg) ? CG_BG_ROUND : 0;
                    cell->bg = (color.alpha == 255 ? colour : blend(cell->bg, colour, float(color.alpha) / 255.f)) |
                               panel | keep_round;
                }
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

    void set_clip(const position& pos, const border_radiuses&) override { clips.push_back(pos); }
    void del_clip() override
    {
        if(!clips.empty()) clips.pop_back();
    }

    void get_viewport(position& vp) const override { vp = position(0, 0, float(cols) * CW, 1000 * CH); }

    void get_media_features(media_features& m) const override
    {
        m.type = media_type_screen;
        m.width = how == Try::desktop ? DESKTOP_PX : float(cols) * CW;
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
        if(tt == text_transform_uppercase) transform_case(text, Case::upper);
        else if(tt == text_transform_lowercase) transform_case(text, Case::lower);
        else if(tt == text_transform_capitalize) transform_case(text, Case::capitalize);
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

// A border is a whole cell, so the box's background stops inside it, and
// HTML-hidden elements take no room.
const char* const user_css = "* { background-clip: padding-box !important; }"
                             "[hidden], template, noscript { display: none !important; }";
} // namespace

struct cg_grid
{
    grid_container container;
    std::vector<std::string> hrefs;

    cg_grid(int cols, Try how) : container(cols, how) {}

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

    void mark_code(const element::ptr& el)
    {
        if(el->is_comment() || el->css().get_display() == display_none) return;
        if(tag_is(el, "code") || tag_is(el, "pre") || tag_is(el, "kbd") || tag_is(el, "samp"))
        {
            std::vector<position> boxes;
            leaf_boxes(el, boxes);
            for(auto& p : boxes)
            {
                auto [r0, r1, c0, c1] = cells_of(p);
                for(int r = r0; r <= r1 && r < container.rows(); r++)
                    for(int c = c0; c <= c1; c++)
                        if(cg_cell* cell = container.at(r, c); cell && (cell->flags & CG_TEXT)) cell->flags |= CG_CODE;
            }

            return;
        }

        for(auto& child : el->children()) mark_code(child);
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
    apply_borders(container, container.frames, container.content_box(), container.lost_border);
}

std::unique_ptr<cg_grid> rendered(const char* html, size_t len, int32_t cols, Try how)
{
    bool loose = how == Try::loose;
    auto grid = std::make_unique<cg_grid>(cols, how);
    grid_container& container = grid->container;
    auto doc =
        document::createFromString(estring(std::string(html, len), encoding::utf_8), &container, master_css, user_css);
    if(!doc) return nullptr;
    hide_invisible(doc->root());
    container.images.label(doc->root(), cols);
    std::vector<element::ptr> buttons;
    {
        Contents contents(doc->root());
        fit_picture_boxes(container.images.pictures(), contents);
        buttons = flatten_buttons(doc->root(), contents);
        unframe_titles(doc->root(), contents);
    }
    // Before the spread sets a list's centred words left: its blocks stay.
    align_blocks(doc->root());
    left_align_spread(doc->root());
    unfloat_cells(doc->root());
    block_regrouped_cells(doc->root());
    // Before the digits are raised: a cell of raised digits keeps no gap.
    gap_cells(doc->root());
    raise_digits(doc->root());
    snap_tree(doc->root());
    // After the snap: a cell under half a row asks for no height.
    rows_as_high_as_cells(doc->root());
    container.body = body_size(doc->root());
    if(loose) loosen(doc->root());
    draw(doc, container);
    // The tables round what still spills are stacked.
    if(loose && container.spilt() && stack_spilt_tables(doc->root(), float(cols) * CW)) draw(doc, container);
    shape_buttons(buttons, container);
    grid->mark_links(doc->root());
    grid->mark_code(doc->root());
    keep_small_print(container);
    return grid;
}

// From the first column the mail shows something in to the last.
int columns_shown(const cg_grid& grid)
{
    Box box = grid.container.content_box();
    return box.c1 < box.c0 ? 0 : box.c1 - box.c0 + 1;
}

// A mail's mobile styles fix widths for a phone, `width: 320px`, and in a
// pane just under their breakpoint its words stood in a column under two
// thirds of it.
bool for_a_phone(const cg_grid& grid)
{
    int shown = columns_shown(grid), cols = grid.container.cols;
    return float(cols) * CW <= PHONE_PX && shown > 0 && shown * 3 < cols * 2;
}

std::unique_ptr<cg_grid> laid_out(const char* html, size_t len, int32_t cols)
{
    auto grid = rendered(html, len, cols, Try::pane);
    if(!grid) return nullptr;
    if(grid->container.spilt()) return rendered(html, len, cols, Try::loose);
    if(!for_a_phone(*grid)) return grid;
    auto wide = rendered(html, len, cols, Try::desktop);
    bool better = wide && !wide->container.spilt() && columns_shown(*wide) > columns_shown(*grid);
    return better ? std::move(wide) : std::move(grid);
}
} // namespace

extern "C" cg_grid* cg_render(const char* html, size_t len, int32_t cols)
{
    try
    {
        return laid_out(html, len, cols).release();
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
