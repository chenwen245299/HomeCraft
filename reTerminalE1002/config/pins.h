#pragma once
// E1002 V1.2 schematic pages4-8 + pinned Seeed examples. Not verified on this physical unit.
// GPIO numbers, never connector positions. Do not include another board's pin configuration.
#define E1002_EPD_SCK 7
#define E1002_EPD_MISO 8
#define E1002_EPD_MOSI 9
#define E1002_EPD_CS 10
#define E1002_EPD_DC 11
#define E1002_EPD_RESET 12
#define E1002_EPD_BUSY 13
#define E1002_SD_CS 14
#define E1002_SD_DETECT 15
#define E1002_SD_ENABLE 16
#define E1002_I2C_SDA 19
#define E1002_I2C_SCL 20
#define E1002_CHARGER_I2C_SDA 39
#define E1002_CHARGER_I2C_SCL 40
#define E1002_UART_TX 43
#define E1002_UART_RX 44
// Physical positions per Seeed's E1002 examples: KEY0 right (green), KEY1 middle, KEY2 left.
#define E1002_KEY0 3
#define E1002_KEY1 4
#define E1002_KEY2 5
#define E1002_LED 6
#define E1002_BUZZER 45
#define E1002_BATTERY_ADC 1
#define E1002_BATTERY_ENABLE 21
#define E1002_MIC_ENABLE 38
#define E1002_PDM_DATA 41
#define E1002_PDM_CLK 42
#define E1002_HEADER_GPIO 46
#define E1002_HEADER_ADC_GPIO 2
#define E1002_SHT40_ADDRESS 0x44
#define E1002_PCF8563_ADDRESS 0x51
#define E1002_DISPLAY_WIDTH 800
#define E1002_DISPLAY_HEIGHT 480
