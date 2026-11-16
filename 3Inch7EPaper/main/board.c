#include "board.h"
#include "driver/gpio.h"
#include "driver/i2c_master.h"
#include "esp_log.h"
#include "esp_rom_sys.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"

static const char *TAG = "board";
static i2c_master_bus_handle_t bus;
static i2c_master_dev_handle_t sensor;
static i2c_master_dev_handle_t pmu;
static i2c_master_dev_handle_t imu;
static bool audio_power_saved;
static uint8_t audio_saved_enable,audio_saved_voltage;
static esp_err_t read_reg(uint8_t reg,uint8_t *value);
static esp_err_t write_reg(uint8_t reg,uint8_t value);

void *board_audio_i2c_bus(void) {return bus;}
esp_err_t board_audio_power(bool enabled)
{
    uint8_t value;esp_err_t err;
    if(enabled&&!audio_power_saved) {
        if((err=read_reg(0x90,&audio_saved_enable))!=ESP_OK)return err;
        if((err=read_reg(0x92,&audio_saved_voltage))!=ESP_OK)return err;
        audio_power_saved=true;
        if((err=write_reg(0x92,(audio_saved_voltage&0xe0)|28))!=ESP_OK)return err;
        if((err=read_reg(0x90,&value))!=ESP_OK)return err;
        if((err=write_reg(0x90,value|1))!=ESP_OK)return err;
        vTaskDelay(pdMS_TO_TICKS(40));
    } else if(!enabled&&audio_power_saved) {
        if((err=read_reg(0x90,&value))!=ESP_OK)return err;
        if((err=write_reg(0x90,(value&~1U)|(audio_saved_enable&1)))!=ESP_OK)return err;
        if((err=write_reg(0x92,audio_saved_voltage))!=ESP_OK)return err;
        audio_power_saved=false;
    }
    return ESP_OK;
}

static esp_err_t read_reg(uint8_t reg, uint8_t *value)
{
    return i2c_master_transmit_receive(pmu, &reg, 1, value, 1, 200);
}

static esp_err_t write_reg(uint8_t reg, uint8_t value)
{
    uint8_t bytes[] = {reg, value};
    return i2c_master_transmit(pmu, bytes, sizeof(bytes), 200);
}

esp_err_t board_init(void)
{
    i2c_master_bus_config_t cfg = {
        .i2c_port = I2C_NUM_0,
        .sda_io_num = 41, .scl_io_num = 42,
        .clk_source = I2C_CLK_SRC_DEFAULT,
        .glitch_ignore_cnt = 7,
        .flags.enable_internal_pullup = true,
    };
    esp_err_t err = i2c_new_master_bus(&cfg, &bus);
    if (err != ESP_OK) return err;
    i2c_device_config_t dev = {
        .dev_addr_length = I2C_ADDR_BIT_LEN_7,
        .device_address = 0x70,
        .scl_speed_hz = 400000,
    };
    err = i2c_master_bus_add_device(bus, &dev, &sensor);
    if (err != ESP_OK) return err;
    dev.device_address = 0x34;
    err = i2c_master_bus_add_device(bus, &dev, &pmu);
    if (err != ESP_OK) return err;
    uint8_t id = 0;
    err = read_reg(0x03, &id);
    if (err != ESP_OK) return err;
    // Upstream uses the AXP2101 register map. Do not modify an unidentified PMU.
    ESP_LOGI(TAG, "PMU chip ID: 0x%02x", id);
    if ((id & 0xCF) != 0x4A) {
        ESP_LOGE(TAG, "Unsupported PMU; screen power configuration skipped");
        return ESP_ERR_NOT_SUPPORTED;
    }
    return board_display_power(true);
}

esp_err_t board_display_power(bool enabled)
{
    uint8_t value;
    esp_err_t err;
    if (enabled) {
        err = read_reg(0x94, &value);
        if (err != ESP_OK) return err;
        // ALDO3 = 3.3 V, preserve unrelated register bits. No charging settings changed.
        err = write_reg(0x94, (value & 0xE0) | 28);
        if (err != ESP_OK) return err;
    }
    err = read_reg(0x90, &value);
    if (err != ESP_OK) return err;
    value = enabled ? (value | (1U << 2)) : (value & ~(1U << 2));
    err = write_reg(0x90, value);
    if (enabled) vTaskDelay(pdMS_TO_TICKS(20));
    return err;
}

esp_err_t board_read_power_source(power_status_t *power)
{
    // Only source/charge flags, without polling the battery gauge or changing PMU settings.
    *power=(power_status_t){.percent=-1};
    uint8_t source,charge;
    esp_err_t err=read_reg(0x00,&source);
    if(err!=ESP_OK)return err;
    err=read_reg(0x01,&charge);
    if(err!=ESP_OK)return err;
    power->valid=true;
    power->usb_present=(source&0x20)!=0;
    power->battery_present=(source&0x08)!=0;
    power->charging=power->usb_present && power->battery_present && ((charge>>5)&3)==1;
    return ESP_OK;
}

esp_err_t board_read_power(power_status_t *power)
{
    // TG28 datasheet: 00[5] VBUS good, 00[3] battery present,
    // 01[6:5] current direction, 18[3] gauge enabled, A4 battery percent.
    // Read only: preserve all existing charger and fuel-gauge configuration.
    *power = (power_status_t){.percent = -1};
    uint8_t status1, status2, modules = 0, percent = 255, confirm;
    esp_err_t err = read_reg(0x00, &status1);
    if (err != ESP_OK) return err;
    err = read_reg(0x01, &status2);
    if (err != ESP_OK) return err;
    if (status1 & 0x08) {
        err = read_reg(0x18, &modules);
        if (err != ESP_OK) return err;
        if (modules & 0x08) {
            err = read_reg(0xA4, &percent);
            if (err != ESP_OK) return err;
        }
    }
    err = read_reg(0x00, &confirm);
    if (err != ESP_OK) return err;
    // Never associate an old SOC value with a battery inserted during this read.
    if ((status1 ^ confirm) & 0x28) return ESP_ERR_INVALID_STATE;
    power->valid = true;
    power->usb_present = (status1 & 0x20) != 0;
    power->battery_present = (status1 & 0x08) != 0;
    power->charging = power->usb_present && power->battery_present && ((status2 >> 5) & 3) == 1;
    if (power->battery_present && (modules & 0x08) && percent <= 100)
        power->percent = percent;
    return ESP_OK;
}

esp_err_t board_power_off(void)
{
    // TG28 datasheet 6.5.4.3 / REG10[0], same operation as upstream shutdown().
    // Recheck ID and preserve charging, reset and other common-config bits.
    uint8_t id, value;
    esp_err_t err = read_reg(0x03, &id);
    if (err != ESP_OK) return err;
    if ((id & 0xCF) != 0x4A) return ESP_ERR_NOT_SUPPORTED;
    err = read_reg(0x10, &value);
    return err == ESP_OK ? write_reg(0x10, value | 1U) : err;
}

static bool crc_ok(const uint8_t *data, uint8_t expected)
{
    uint8_t crc = 0xFF;
    for (unsigned i = 0; i < 2; ++i) {
        crc ^= data[i];
        for (unsigned bit = 0; bit < 8; ++bit)
            crc = (crc & 0x80) ? (uint8_t)((crc << 1) ^ 0x31) : (uint8_t)(crc << 1);
    }
    return crc == expected;
}

esp_err_t board_read_environment(float *temperature, float *humidity)
{
    const uint8_t wake[] = {0x35, 0x17};
    const uint8_t measure[] = {0x78, 0x66};
    const uint8_t sleep[] = {0xB0, 0x98};
    uint8_t raw[6];
    esp_err_t err = i2c_master_transmit(sensor, wake, sizeof(wake), 200);
    if (err != ESP_OK) return err;
    esp_rom_delay_us(300);
    err = i2c_master_transmit(sensor, measure, sizeof(measure), 200);
    if (err == ESP_OK) {
        vTaskDelay(pdMS_TO_TICKS(20));
        err = i2c_master_receive(sensor, raw, sizeof(raw), 200);
    }
    i2c_master_transmit(sensor, sleep, sizeof(sleep), 200);
    if (err != ESP_OK) return err;
    if (!crc_ok(raw, raw[2]) || !crc_ok(raw + 3, raw[5])) return ESP_ERR_INVALID_CRC;
    *temperature = -45.0f + 175.0f * ((raw[0] << 8) | raw[1]) / 65536.0f;
    *humidity = 100.0f * ((raw[3] << 8) | raw[4]) / 65536.0f;
    return ESP_OK;
}

esp_err_t board_inputs_init(void)
{
    // Electrically disable every wheel input, including internal pulls and interrupts.
    gpio_config_t wheel = {.pin_bit_mask = (1ULL << 4) | (1ULL << 5) | (1ULL << 6),
                          .mode = GPIO_MODE_DISABLE, .intr_type = GPIO_INTR_DISABLE};
    esp_err_t err = gpio_config(&wheel);
    if (err != ESP_OK) return err;
    gpio_config_t boot = {.pin_bit_mask = 1ULL << 0, .mode = GPIO_MODE_INPUT,
                         .pull_up_en = GPIO_PULLUP_ENABLE, .intr_type = GPIO_INTR_DISABLE};
    err = gpio_config(&boot);
    if (err != ESP_OK) return err;
    uint8_t value;
    err = read_reg(0x41, &value);
    if (err != ESP_OK) return err;
    err = write_reg(0x41, value | 0x0F); // TG28: PWR short/long and both edges.
    if (err != ESP_OK) return err;
    err = write_reg(0x49, 0x0F); // W1C key IRQs only; preserve unrelated status.
    if (err != ESP_OK) return err;
    err = read_reg(0x27, &value);
    if (err != ESP_OK) return err;
    err = write_reg(0x27, (value & ~0x0C) | 0x08); // 8-second hardware hold; preserve on-time.
    if (err != ESP_OK) return err;
    err = read_reg(0x22, &value);
    if (err != ESP_OK) return err;
    return write_reg(0x22, (value | 0x02) & ~0x01); // Keep hardware hold-to-power-off, not restart.
}

bool board_boot_down(void) { return gpio_get_level(GPIO_NUM_0) == 0; }

esp_err_t board_power_key_irq(uint8_t *events)
{
    uint8_t flags;
    esp_err_t err = read_reg(0x49, &flags);
    if (err != ESP_OK) return err;
    *events = flags & 0x0F;
    if (*events) return write_reg(0x49, *events);
    return ESP_OK;
}

static esp_err_t imu_read(uint8_t reg, uint8_t *data, size_t size)
{
    if (!imu) return ESP_ERR_NOT_FOUND;
    return i2c_master_transmit_receive(imu, &reg, 1, data, size, 100);
}

static esp_err_t imu_write(uint8_t reg, uint8_t value)
{
    uint8_t bytes[] = {reg, value};
    return i2c_master_transmit(imu, bytes, sizeof(bytes), 100);
}

esp_err_t board_imu_init(void)
{
    for (unsigned address = 0x6A; address <= 0x6B; ++address) {
        if (i2c_master_probe(bus, address, 100) != ESP_OK) continue;
        i2c_device_config_t dev = {.dev_addr_length = I2C_ADDR_BIT_LEN_7,
            .device_address = address, .scl_speed_hz = 400000};
        esp_err_t err = i2c_master_bus_add_device(bus, &dev, &imu);
        if (err != ESP_OK) return err;
        uint8_t id = 0;
        err = imu_read(0x00, &id, 1);
        if (err == ESP_OK && id == 0x05) {
            ESP_LOGI(TAG, "EPAPER_IMU QMI8658 address=0x%02x id=0x%02x", address, id);
            if ((err = imu_write(0x08, 0x00)) != ESP_OK) return err;
            if ((err = imu_write(0x02, 0x60)) != ESP_OK) return err; // Increment, little-endian.
            // QST datasheet CTRL2: +/-2g, 11 Hz low-power accel-only (0b1110).
            if ((err = imu_write(0x03, 0x0E)) != ESP_OK) return err;
            if ((err = imu_write(0x06, 0x00)) != ESP_OK) return err;
            if ((err = imu_write(0x08, 0x01)) != ESP_OK) return err; // No GPIO39/40 interrupt routing.
            uint8_t ctrl2,ctrl7;
            if ((err = board_read_imu_config(&ctrl2,&ctrl7)) != ESP_OK) return err;
            if (ctrl2 != 0x0E || ctrl7 != 0x01) return ESP_ERR_INVALID_RESPONSE;
            ESP_LOGI(TAG,"EPAPER_IMU_MODE ctrl2=0x%02x ctrl7=0x%02x accel_hz=11 low_power=1",ctrl2,ctrl7);
            return ESP_OK;
        }
        i2c_master_bus_rm_device(imu);
        imu = NULL;
    }
    return ESP_ERR_NOT_FOUND;
}

esp_err_t board_read_acceleration(float xyz[3])
{
    uint8_t data[6];
    esp_err_t err = imu_read(0x35, data, sizeof(data));
    if (err != ESP_OK) return err;
    for (unsigned i = 0; i < 3; ++i)
        xyz[i] = (int16_t)((uint16_t)data[i*2] | ((uint16_t)data[i*2+1] << 8)) / 16384.0f;
    return ESP_OK;
}

esp_err_t board_read_imu_config(uint8_t *ctrl2,uint8_t *ctrl7)
{
    esp_err_t err=imu_read(0x03,ctrl2,1);
    return err==ESP_OK?imu_read(0x08,ctrl7,1):err;
}
