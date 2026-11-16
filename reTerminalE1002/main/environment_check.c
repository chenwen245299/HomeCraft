#include <stdio.h>
#include "esp_system.h"
#include "esp_flash.h"
#include "esp_psram.h"
#include "pins.h"
_Static_assert(E1002_UART_TX==43 && E1002_UART_RX==44,"USB-UART profile mismatch");
_Static_assert(E1002_I2C_SDA==19 && E1002_I2C_SCL==20,"E1002 sensor bus mismatch");
void app_main(void)
{
    uint32_t flash_size=0;
    esp_err_t err=esp_flash_get_size(NULL,&flash_size);
    printf("E1002 environment-only build: IDF=%s flash_read_ok=%d flash_bytes=%lu psram_bytes=%u\n",
        esp_get_idf_version(),err==ESP_OK,(unsigned long)flash_size,(unsigned)esp_psram_get_size());
    // No display, SD, sensor, charger, microphone, radio or HomeCraft implementation.
}
