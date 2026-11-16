// SPDX-License-Identifier: GPL-3.0-or-later
// ESP-IDF port of Seeed_GxEPD2 GxEPD2_730c_GDEP073E01 (Jean-Marc Zingg).
// Pinned upstream1100ea37c16b910fd79152f4250c13d802b9c20b; see THIRD_PARTY.md.
// Preserve vendor waveform/settings; only transport and bounded window validation differ.
#include "e1002_display.h"
#include "pins.h"
#include "driver/gpio.h"
#include "driver/spi_master.h"
#include "esp_timer.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include <string.h>
static spi_device_handle_t spi;
static bool initialized,powered;
static void delay_ms(unsigned ms){vTaskDelay(pdMS_TO_TICKS(ms?ms:1));}
static esp_err_t bytes(const void *data,size_t size,bool dc){
 gpio_set_level(E1002_EPD_DC,dc);
 spi_transaction_t t={.length=size*8,.tx_buffer=data};
 return spi_device_polling_transmit(spi,&t);
}
static esp_err_t cmd(uint8_t c){return bytes(&c,1,false);}
static esp_err_t reg(uint8_t c,const uint8_t *v,size_t n){esp_err_t e=cmd(c);return e==ESP_OK?bytes(v,n,true):e;}
static esp_err_t wait_busy(void){
 int64_t start=esp_timer_get_time();delay_ms(5);
 while(!gpio_get_level(E1002_EPD_BUSY)){
  if(esp_timer_get_time()-start>60000000)return ESP_ERR_TIMEOUT;
  delay_ms(20);
 }return ESP_OK;
}
static esp_err_t window(e1002_rect_t r){
 uint16_t xe=r.x+r.w-1,ye=r.y+r.h; // Vendor's one-extra-line controller workaround.
 uint8_t b[]={r.x>>8,r.x,xe>>8,xe,r.y>>8,r.y,ye>>8,ye,1};
 return reg(0x83,b,sizeof(b));
}
#define CHECK(call) do{esp_err_t e=(call);if(e!=ESP_OK)return e;}while(0)
static esp_err_t power_on(void){if(!powered){CHECK(cmd(0x04));CHECK(wait_busy());powered=true;}return ESP_OK;}
esp_err_t e1002_display_init(void){
 gpio_config_t out={.pin_bit_mask=(1ULL<<E1002_EPD_DC)|(1ULL<<E1002_EPD_RESET)|(1ULL<<E1002_SD_CS),.mode=GPIO_MODE_OUTPUT};
 CHECK(gpio_config(&out));gpio_set_level(E1002_SD_CS,1);
 gpio_config_t in={.pin_bit_mask=1ULL<<E1002_EPD_BUSY,.mode=GPIO_MODE_INPUT};CHECK(gpio_config(&in));
 spi_bus_config_t bus={.mosi_io_num=E1002_EPD_MOSI,.miso_io_num=E1002_EPD_MISO,.sclk_io_num=E1002_EPD_SCK,.quadwp_io_num=-1,.quadhd_io_num=-1,.max_transfer_sz=4096};
 CHECK(spi_bus_initialize(SPI2_HOST,&bus,SPI_DMA_CH_AUTO));
 spi_device_interface_config_t device={.clock_speed_hz=2000000,.mode=0,.spics_io_num=E1002_EPD_CS,.queue_size=1};
 CHECK(spi_bus_add_device(SPI2_HOST,&device,&spi));return ESP_OK;
}
static esp_err_t controller_init(void){
 gpio_set_level(E1002_EPD_RESET,1);delay_ms(50);gpio_set_level(E1002_EPD_RESET,0);delay_ms(20);gpio_set_level(E1002_EPD_RESET,1);delay_ms(10);
 static const struct {uint8_t c,n,b[6];} settings[]={
 {0xAA,6,{0x49,0x55,0x20,0x08,0x09,0x18}}, {0x01,1,{0x3F}}, {0x00,2,{0x5F,0x69}},
 {0x03,4,{0,0x54,0,0x44}}, {0x05,4,{0x40,0x1F,0x1F,0x2C}}, {0x06,4,{0x6F,0x1F,0x17,0x49}},
 {0x08,4,{0x6F,0x1F,0x1F,0x22}}, {0x30,1,{0x08}}, {0x50,1,{0x3F}}, {0x60,2,{0x02,0}},
 {0x61,4,{0x03,0x20,0x01,0xE0}}, {0x84,1,{1}}, {0xE3,1,{0x2F}}};
 for(unsigned i=0;i<sizeof(settings)/sizeof(settings[0]);++i)CHECK(reg(settings[i].c,settings[i].b,settings[i].n));
 powered=false;CHECK(power_on());initialized=true;return ESP_OK;
}
esp_err_t e1002_display_present(const uint8_t *frame,const e1002_rect_t *region,uint32_t *elapsed_ms){
 if(!spi||!frame)return ESP_ERR_INVALID_STATE;
 e1002_rect_t r=region?*region:(e1002_rect_t){0,0,800,480};
 // Partial windows must sit on 8-pixel columns, as in the vendor driver's writeImage path.
 if(r.x<0||r.y<0||r.w<=0||r.h<=0||r.x+r.w>800||r.y+r.h>480||(r.x&7)||(r.w&7))return ESP_ERR_INVALID_ARG;
 if(!initialized&&region)return ESP_ERR_INVALID_STATE; // First frame must establish all controller RAM.
 int64_t start=esp_timer_get_time();
 if(!initialized)CHECK(controller_init());
 CHECK(power_on());CHECK(window(r));CHECK(cmd(0x10));
 for(int y=r.y;y<r.y+r.h;++y)CHECK(bytes(frame+y*400+r.x/2,r.w/2,true));
 // The refresh window is programmed again, not merely the RAM write rectangle.
 CHECK(window(r));uint8_t border=region?0xFF:0x3F;CHECK(reg(0x50,&border,1));
 uint8_t zero=0;CHECK(reg(0x12,&zero,1));CHECK(wait_busy());
 CHECK(reg(0x02,&zero,1));CHECK(wait_busy());powered=false;
 if(elapsed_ms)*elapsed_ms=(esp_timer_get_time()-start)/1000;
 return ESP_OK;
}
