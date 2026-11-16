#include "audio_output.h"
#include "board.h"
#include "driver/gpio.h"
#include "driver/i2s_std.h"
#include "esp_codec_dev.h"
#include "esp_codec_dev_defaults.h"
#include "esp_pm.h"
#include "esp_check.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include <stdio.h>
static i2s_chan_handle_t tx;
static esp_codec_dev_handle_t player;
static esp_pm_lock_handle_t cpu_lock,sleep_lock;
static esp_pm_config_t saved_pm;
static bool active,pm_active;
static bool tx_needs_enable;

void audio_output_init(void)
{
    gpio_config_t pa={.pin_bit_mask=1ULL<<39,.mode=GPIO_MODE_OUTPUT};
    gpio_config(&pa);gpio_set_level(39,0);
}
static esp_err_t create_output(void)
{
    if(player)return ESP_OK;
    esp_err_t ret=ESP_OK;bool enabled=false;
    const audio_codec_ctrl_if_t *control=NULL;
    const audio_codec_if_t *codec=NULL;
    const audio_codec_data_if_t *data=NULL;
    i2s_chan_config_t channel=I2S_CHANNEL_DEFAULT_CONFIG(I2S_NUM_0,I2S_ROLE_MASTER);channel.auto_clear=true;
    ESP_GOTO_ON_ERROR(i2s_new_channel(&channel,&tx,NULL),fail,"music","I2S allocation");
    i2s_std_config_t cfg={.clk_cfg=I2S_STD_CLK_DEFAULT_CONFIG(44100),
        .slot_cfg=I2S_STD_PHILIPS_SLOT_DEFAULT_CONFIG(I2S_DATA_BIT_WIDTH_16BIT,I2S_SLOT_MODE_STEREO),
        .gpio_cfg={.mclk=13,.bclk=14,.ws=47,.dout=48,.din=I2S_GPIO_UNUSED}};
    ESP_GOTO_ON_ERROR(i2s_channel_init_std_mode(tx,&cfg),fail,"music","I2S mode");
    ESP_GOTO_ON_ERROR(i2s_channel_enable(tx),fail,"music","I2S initial enable");enabled=true;tx_needs_enable=false;
    audio_codec_i2c_cfg_t ic={.port=0,.addr=ES8311_CODEC_DEFAULT_ADDR,.bus_handle=board_audio_i2c_bus()};
    control=audio_codec_new_i2c_ctrl(&ic);
    es8311_codec_cfg_t cc={.ctrl_if=control,.codec_mode=ESP_CODEC_DEV_WORK_MODE_DAC,
        .pa_pin=-1,.use_mclk=true,.mclk_div=256,.hw_gain={.pa_voltage=3.3,.codec_dac_voltage=3.3}};
    codec=control?es8311_codec_new(&cc):NULL;
    audio_codec_i2s_cfg_t dc={.port=I2S_NUM_0,.tx_handle=tx};
    data=codec?audio_codec_new_i2s_data(&dc):NULL;
    if(!codec||!data){ret=ESP_ERR_NO_MEM;goto fail;}
    esp_codec_dev_cfg_t dev={.codec_if=codec,.data_if=data,.dev_type=ESP_CODEC_DEV_TYPE_OUT};
    player=esp_codec_dev_new(&dev);if(player)return ESP_OK;
    ret=ESP_ERR_NO_MEM;
fail:
    if(data)audio_codec_delete_data_if(data);
    if(codec)audio_codec_delete_codec_if(codec);
    if(control)audio_codec_delete_ctrl_if(control);
    if(tx){if(enabled)i2s_channel_disable(tx);i2s_del_channel(tx);tx=NULL;tx_needs_enable=false;}
    return ret;
}
void audio_output_close(void)
{
    bool was_active=active;
    if(active&&player){esp_codec_dev_set_out_mute(player,true);esp_codec_dev_set_out_vol(player,0);}
    gpio_set_level(39,0);
    if(player)esp_codec_dev_close(player);
    if(was_active)tx_needs_enable=true;
    active=false;esp_err_t power_err=board_audio_power(false);
    if(pm_active) {
        esp_pm_lock_release(sleep_lock);esp_pm_lock_release(cpu_lock);
        esp_pm_configure(&saved_pm);pm_active=false;
    }
    if(was_active)printf("EPAPER_MUSIC_OUTPUT closed=1 pa=%d power_restored=%d\n",gpio_get_level(39),power_err==ESP_OK);
}
esp_err_t audio_output_open(unsigned rate,unsigned volume)
{
    if(active)return ESP_OK;
    esp_err_t err=board_audio_power(true);if(err!=ESP_OK)goto fail;
    if((err=create_output())!=ESP_OK)goto fail;
    // esp_codec_dev reconfigures a running I2S channel on open; close leaves it disabled.
    if(tx_needs_enable) {
        if((err=i2s_channel_enable(tx))!=ESP_OK)goto fail;
        tx_needs_enable=false;
    }
    if(!cpu_lock) {
        if((err=esp_pm_lock_create(ESP_PM_CPU_FREQ_MAX,0,"music_cpu",&cpu_lock))!=ESP_OK)goto fail;
    }
    if(!sleep_lock&&(err=esp_pm_lock_create(ESP_PM_NO_LIGHT_SLEEP,0,"music_i2s",&sleep_lock))!=ESP_OK)goto fail;
    esp_pm_get_configuration(&saved_pm);esp_pm_config_t pm=saved_pm;pm.max_freq_mhz=160;
    if((err=esp_pm_configure(&pm))!=ESP_OK)goto fail;
    esp_pm_lock_acquire(cpu_lock);esp_pm_lock_acquire(sleep_lock);pm_active=true;
    esp_codec_dev_sample_info_t fs={.sample_rate=rate,.channel=2,.bits_per_sample=16};
    err=esp_codec_dev_open(player,&fs);if(err!=ESP_OK)goto fail;
    active=true;esp_codec_dev_set_out_mute(player,true);esp_codec_dev_set_out_vol(player,volume);
    // Start with silence before enabling the amplifier; no microphone is opened.
    int16_t silence[1024]={0};esp_codec_dev_write(player,silence,sizeof(silence));
    esp_codec_dev_set_out_mute(player,false);gpio_set_level(39,1);return ESP_OK;
fail:audio_output_close();return err;
}
esp_err_t audio_output_write(const int16_t *stereo,size_t frames)
{
    if(!active||frames>512)return ESP_ERR_INVALID_STATE;
    return esp_codec_dev_write(player,(void *)stereo,(int)(frames*4));
}
void audio_output_volume(unsigned volume)
{
    if(active&&player)esp_codec_dev_set_out_vol(player,volume>100?100:volume);
}
