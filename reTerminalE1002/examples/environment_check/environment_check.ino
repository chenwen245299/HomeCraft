// Compile-only environment check; no application feature selection.
#include <Arduino.h>
#include "pins.h"
static_assert(E1002_UART_TX==43 && E1002_I2C_SDA==19,"E1002 profile required");
void setup() { Serial.begin(115200); Serial.println("E1002 environment-only build"); }
void loop() { delay(1000); }
