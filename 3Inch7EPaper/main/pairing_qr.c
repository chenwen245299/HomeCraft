#include "pairing_qr.h"
#include "qrcodegen.h"

bool canvas_pairing_qr(canvas_t *c, int x, int y, int box_size, const char *payload)
{
    // Use the pure encoder: Espressif's convenience wrapper logs the QR payload.
    uint8_t temp[qrcodegen_BUFFER_LEN_FOR_VERSION(10)];
    uint8_t code[qrcodegen_BUFFER_LEN_FOR_VERSION(10)];
    if (!qrcodegen_encodeText(payload, temp, code, qrcodegen_Ecc_MEDIUM,
                             1, 10, qrcodegen_Mask_AUTO, true)) return false;
    int n = qrcodegen_getSize(code), scale = box_size / (n + 8);
    if (scale < 1) return false;
    int offset = (box_size - (n + 8) * scale) / 2;
    for (int row = 0; row < n; ++row)
        for (int col = 0; col < n; ++col)
            if (qrcodegen_getModule(code, col, row))
                canvas_fill(c, x + offset + (col + 4)*scale,
                            y + offset + (row + 4)*scale, scale, scale, true);
    return true;
}
