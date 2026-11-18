#include "colour.h"

#include "cell_grid.h"

#include <algorithm>
#include <cmath>
#include <cstdlib>

namespace cg
{
namespace
{
uint32_t channel(uint32_t c, int shift) { return (c >> shift) & 255; }
} // namespace

uint32_t rgb(const litehtml::web_color& c)
{
    return 0x01000000u | (uint32_t(c.red) << 16) | (uint32_t(c.green) << 8) | c.blue;
}

uint32_t blend(uint32_t under, uint32_t over, float a)
{
    if(!under) under = 0x01FFFFFF;
    auto mix = [a](uint32_t u, uint32_t o) {
        return uint32_t(std::lround(float(u) + (float(o) - float(u)) * a));
    };
    uint32_t r = mix(channel(under, 16), channel(over, 16));
    uint32_t g = mix(channel(under, 8), channel(over, 8));
    uint32_t b = mix(channel(under, 0), channel(over, 0));
    return 0x01000000u | (r << 16) | (g << 8) | b;
}

bool page_light(uint32_t c)
{
    auto lin = [](uint32_t v) {
        float x = float(v) / 255.f;
        return x <= 0.04045f ? x / 12.92f : std::pow((x + 0.055f) / 1.055f, 2.4f);
    };
    float luminance = 0.2126f * lin(channel(c, 16)) + 0.7152f * lin(channel(c, 8)) + 0.0722f * lin(channel(c, 0));
    return luminance > CG_PAGE_LUMINANCE;
}

bool grey(uint32_t c)
{
    auto [low, high] = std::minmax({channel(c, 16), channel(c, 8), channel(c, 0)});
    return high - low < CG_GREY_SPREAD;
}

bool alike(uint32_t a, uint32_t b)
{
    for(int shift : {16, 8, 0})
        if(std::abs(int(channel(a, shift)) - int(channel(b, shift))) > 25) return false;
    return true;
}
} // namespace cg
