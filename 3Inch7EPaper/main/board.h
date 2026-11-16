#pragma once
#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"
#include "power_status.h"

esp_err_t board_init(void);
esp_err_t board_display_power(bool enabled);
esp_err_t board_power_off(void);
esp_err_t board_read_environment(float *temperature, float *humidity);
esp_err_t board_read_power(power_status_t *power);
esp_err_t board_read_power_source(power_status_t *power);
esp_err_t board_inputs_init(void);
esp_err_t board_power_key_irq(uint8_t *events);
bool board_boot_down(void);
esp_err_t board_imu_init(void);
esp_err_t board_read_acceleration(float xyz[3]);
esp_err_t board_read_imu_config(uint8_t *ctrl2,uint8_t *ctrl7);
void *board_audio_i2c_bus(void);
esp_err_t board_audio_power(bool enabled);
