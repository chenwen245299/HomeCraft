idf_component_register(
    SRCS "main.c" "energy_policy.c" "board.c" "dashboard.c" "quota_reset.c" "argus_page.c" "token_typography.c" "canvas.c" "fonts.c" "brand_assets.c" "protocol.c" "controls.c" "network.c" "usage.c" "harness_stats.c" "lunar_calendar.c" "usage_breakdown.c" "pairing_qr.c" "environment_log.c" "environment_csv.c" "refresh_policy.c" "sample_refresh.c" "shutdown.c" "ble_enrollment.c" "device_protocol.c"
    INCLUDE_DIRS "."
    REQUIRES bt epaper_vendor esp_driver_gpio esp_driver_i2c esp_driver_usb_serial_jtag esp_timer esp_psram esp_app_format fatfs sdmmc esp_driver_sdmmc
             esp_wifi esp_netif nvs_flash esp_event esp_http_client esp_http_server mbedtls lwip espressif__cjson
             espressif__network_provisioning espressif__qrcode protocomm esp_pm
             espressif__esp_codec_dev espressif__esp_audio_codec esp_driver_i2s
    EMBED_TXTFILES "settings_page.html"
)
target_sources(${COMPONENT_LIB} PRIVATE "ble_recovery.c" "ble_transport_probe.c" "power_events.c")
target_sources(${COMPONENT_LIB} PRIVATE "music_library.c" "music_pcm.c" "music.c" "audio_output.c" "music_page.c" "music_fonts.c")
idf_component_get_property(qrcode_dir espressif__qrcode COMPONENT_DIR)
target_include_directories(${COMPONENT_LIB} PRIVATE "${qrcode_dir}")
