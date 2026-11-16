idf_component_register(SRCS "main.c" "ble_link.c" "color_canvas.c" "dashboard.c" "fonts.c" "logos.c" "icons.c" "lunar.c" "privacy.c" "usage.c" "protocol.c"
 INCLUDE_DIRS "." "../config" REQUIRES esp_psram spi_flash e1002_display esp_driver_gpio esp_driver_i2c esp_adc esp_timer esp_event nvs_flash protocomm bt cjson bootloader_support)
