#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <string.h>
#include "controls.h"
#include "canvas.h"

static uint64_t now;
static gesture_t events[16];
static unsigned count;
static button_state_t button;

static void ticks(bool down, unsigned duration)
{
    for (unsigned i = 0; i < duration; i += 10, now += 10) {
        gesture_t event = button_update(&button, down, now);
        if (event) { assert(count < 16); events[count++] = event; }
    }
}

static void reset_button(void)
{
    memset(&button, 0, sizeof(button)); count = 0; now = 0;
}

static void click(void) { ticks(true, 80); ticks(false, 100); }

static void hold_orientation(orientation_filter_t *f, float x, float y, float z)
{
    for (unsigned i = 0; i < 40; ++i, now += 100) orientation_update(f, x, y, z, now);
}

static bool black(const uint8_t *pixels, int x, int y)
{
    return !(pixels[y * 100 + x / 8] & (0x80 >> (x % 8)));
}

int main(void)
{
    reset_button(); ticks(true, 10); ticks(false, 500); assert(count == 0);
    reset_button(); click(); ticks(false, 500); assert(count == 1 && events[0] == GESTURE_SINGLE);
    reset_button(); click(); click(); ticks(false, 500); assert(count == 1 && events[0] == GESTURE_DOUBLE);
    reset_button(); click(); click(); click(); ticks(false, 500); assert(count == 1 && events[0] == GESTURE_TRIPLE);
    reset_button(); ticks(true, 3000); ticks(false, 700); assert(count == 1 && events[0] == GESTURE_LONG);
    reset_button(); click(); ticks(true, 1500); ticks(false, 700); assert(count == 1 && events[0] == GESTURE_LONG);

    // BOOT uses a separate 3-second threshold; it must not open setup at 1.2s.
    button_state_t boot = {0};
    for (unsigned ms = 0; ms < 3000; ms += 10)
        assert(button_update_with_hold(&boot, true, ms, 3000) == GESTURE_NONE);
    assert(button_update_with_hold(&boot, true, 3020, 3000) == GESTURE_LONG);
    for (unsigned ms = 3030; ms < 3500; ms += 10)
        assert(button_update_with_hold(&boot, false, ms, 3000) == GESTURE_NONE);

    power_button_t power={0};
    for(unsigned ms=0;ms<1500;ms+=10)assert(power_button_update(&power,true,ms)==GESTURE_NONE);
    assert(power_button_update(&power,false,1500)==GESTURE_NONE);
    assert(power_button_update(&power,false,1520)==GESTURE_LONG); // Pause only after release.
    for(unsigned ms=1530;ms<2100;ms+=10)assert(power_button_update(&power,false,ms)==GESTURE_NONE);
    power=(power_button_t){0};
    for(unsigned ms=0;ms<3020;ms+=10)assert(power_button_update(&power,true,ms)==GESTURE_NONE);
    assert(power_button_update(&power,true,3020)==GESTURE_SHUTDOWN);
    for(unsigned ms=3030;ms<8000;ms+=10)assert(power_button_update(&power,true,ms)==GESTURE_NONE);
    for(unsigned ms=8000;ms<8600;ms+=10)assert(power_button_update(&power,false,ms)==GESTURE_NONE);
    // Aborted/retried shutdown can be requested again, and a very short tap stays a tap.
    for(unsigned ms=8600;ms<11620;ms+=10)assert(power_button_update(&power,true,ms)==GESTURE_NONE);
    assert(power_button_update(&power,true,11620)==GESTURE_SHUTDOWN);
    power=(power_button_t){0};
    for(unsigned ms=0;ms<80;ms+=10)assert(power_button_update(&power,true,ms)==GESTURE_NONE);
    for(unsigned ms=80;ms<500;ms+=10)assert(power_button_update(&power,false,ms)==GESTURE_NONE);
    assert(power_button_update(&power,false,500)==GESTURE_SINGLE);

    orientation_filter_t f = {0}; now = 0;
    hold_orientation(&f, 0, 1, 0); assert(f.rotation == 0);
    for (unsigned i = 0; i < 8; ++i, now += 100) orientation_update(&f, -1, 0, 0, now);
    assert(f.rotation == 0);
    hold_orientation(&f, -1, 0, 0); assert(f.rotation == 90);
    hold_orientation(&f, 0, -1, 0); assert(f.rotation == 180);
    hold_orientation(&f, 1, 0, 0); assert(f.rotation == 270);
    hold_orientation(&f, 0, 0, 1); assert(f.rotation == 270);
    hold_orientation(&f, .707f, .707f, 0); assert(f.rotation == 270);
    hold_orientation(&f, 2, 2, 0); assert(f.rotation == 270);
    hold_orientation(&f, NAN, 0, 0); assert(f.rotation == 270);

    orientation_filter_t slow={0};
    orientation_update(&slow,0,1,0,0);
    orientation_update(&slow,-1,0,0,1000);assert(slow.rotation==0);
    orientation_update(&slow,-1,0,0,2000);assert(slow.rotation==90);
    orientation_update(&slow,0,0,1,3000);assert(slow.rotation==90);

    uint8_t pixels[SCREEN_BYTES + 32];
    memset(pixels, 0xA5, sizeof(pixels));
    canvas_t c = {.pixels = pixels + 16};
    const unsigned rotations[] = {0, 90, 180, 270};
    const int px[] = {0, 799, 799, 0}, py[] = {0, 0, 479, 479};
    for (unsigned i = 0; i < 4; ++i) {
        c.rotation = rotations[i]; canvas_clear(&c); canvas_pixel(&c, 0, 0, true);
        assert(black(c.pixels, px[i], py[i]));
        canvas_pixel(&c, -1, 0, true);
        canvas_pixel(&c, canvas_width(&c), canvas_height(&c), true);
        for (unsigned j = 0; j < 16; ++j)
            assert(pixels[j] == 0xA5 && pixels[SCREEN_BYTES + 16 + j] == 0xA5);
    }
    puts("Gesture, orientation stability, and four-way canvas mapping: OK");
    return 0;
}
