#include "controls.h"
#include <math.h>

gesture_t button_update_with_hold(button_state_t *b, bool raw_down, uint64_t now, unsigned hold_ms)
{
    if (raw_down != b->raw_down) {
        b->raw_down = raw_down;
        b->changed_ms = now;
    }
    if (b->down != raw_down && now - b->changed_ms >= 20) {
        b->down = raw_down;
        if (raw_down) {
            b->pressed_ms = now;
            b->long_sent = false;
        } else {
            b->released_ms = now;
            if (b->long_sent) b->clicks = 0;
            else if (now >= b->ignore_until_ms && ++b->clicks == 3) {
                b->clicks = 0;
                b->ignore_until_ms = now + 400;
                return GESTURE_TRIPLE;
            }
        }
    }
    if (b->down && !b->long_sent && now - b->pressed_ms >= hold_ms) {
        b->long_sent = true;
        b->clicks = 0;
        return GESTURE_LONG;
    }
    if (!b->down && !raw_down && b->clicks && now - b->released_ms >= 400) {
        gesture_t event = b->clicks == 1 ? GESTURE_SINGLE : GESTURE_DOUBLE;
        b->clicks = 0;
        return event;
    }
    return GESTURE_NONE;
}

gesture_t button_update(button_state_t *b, bool raw_down, uint64_t now)
{
    return button_update_with_hold(b, raw_down, now, 1200);
}

gesture_t power_button_update(power_button_t *p, bool raw_down, uint64_t now)
{
    bool was_down = p->button.down;
    gesture_t gesture = button_update(&p->button, raw_down, now);
    if (!was_down && p->button.down) p->pause_pending = p->shutdown_sent = false;
    if (gesture == GESTURE_LONG) { p->pause_pending = true; gesture = GESTURE_NONE; }
    if (p->button.down && raw_down && !p->shutdown_sent && now - p->button.pressed_ms >= 3000) {
        p->shutdown_sent = true;
        p->pause_pending = false;
        return GESTURE_SHUTDOWN;
    }
    if (was_down && !p->button.down && p->pause_pending) {
        p->pause_pending = false;
        return GESTURE_LONG;
    }
    return gesture;
}

bool orientation_update(orientation_filter_t *f, float x, float y, float z, uint64_t now)
{
    if (!isfinite(x) || !isfinite(y) || !isfinite(z)) return false;
    float norm = x*x + y*y + z*z;
    if (norm < 0.55f || norm > 1.6f) { f->initialized = false; return false; }
    if (!f->initialized) {
        f->x = x; f->y = y; f->z = z;
        f->candidate = f->rotation;
        f->candidate_since_ms = now;
        f->initialized = true;
    } else {
        // Keep the same settling time when the input task samples less often.
        float dt=now>=f->sampled_ms?(float)(now-f->sampled_ms):100.0f;
        float alpha=1.0f-expf(-fminf(dt,1000.0f)/350.0f);
        f->x += alpha * (x - f->x);
        f->y += alpha * (y - f->y);
        f->z += alpha * (z - f->z);
    }
    f->sampled_ms=now;
    float ax = fabsf(f->x), ay = fabsf(f->y);
    unsigned candidate;
    if (ax > 0.55f && ax > ay * 1.4f) candidate = f->x > 0 ? 270 : 90;
    else if (ay > 0.55f && ay > ax * 1.4f) candidate = f->y > 0 ? 0 : 180;
    else {
        f->candidate_since_ms = now; // Flat, diagonal, or moving: keep the current orientation.
        return false;
    }
    if (candidate != f->candidate) {
        f->candidate = candidate;
        f->candidate_since_ms = now;
    }
    if (candidate != f->rotation && now - f->candidate_since_ms >= 1000) {
        f->rotation = candidate;
        return true;
    }
    return false;
}
