use embassy_nrf::gpio::{Input, Level, Output, OutputDrive, Pull};
use embassy_nrf::peripherals::{P0_06, P0_08, P0_16, TEMP, TWISPI0, UARTE0};
use embassy_nrf::Peri;
use embassy_nrf::Peripherals;

pub struct LedPins {
    pub rows: [Output<'static>; 5],
    pub cols: [Output<'static>; 5],
}

/// Пины кнопок A/B (schematic p.3). Active-low, внутренняя подтяжка вверх
/// (внутренний pull-up ~13кОм, nRF52833 PS §6.8.2 GPIO p.144).
pub struct ButtonPins {
    pub a: Input<'static>,
    pub b: Input<'static>,
}

/// Внутренняя I²C-шина (schematic «Target MCU», net I2C_INT).
/// SCL=P0.08, SDA=P0.16, внешние pull-up 1кОм (R51/R52, schematic).
pub struct I2cPins {
    pub twim: Peri<'static, TWISPI0>,
    pub scl: Peri<'static, P0_08>,
    pub sda: Peri<'static, P0_16>,
}

/// UART к интерфейсному MCU (DAPLink CDC). Только TX: шлём телеметрию.
/// Наш TX (nRF TXD) = **P0.06** — по эталонному nrf-rs/microbit BSP.
/// ВНИМАНИЕ: имена нетов на схеме (UART_INT_TX=P1.08, UART_INT_RX=P0.06)
/// заданы с точки зрения ИНТЕРФЕЙСНОГО MCU: его RX (UART_INT_RX, P0.06) =
/// наш TX. Поэтому шлём на P0.06, RX интерфейса = P1.08 (нам не нужен).
pub struct UartPins {
    pub uarte: Peri<'static, UARTE0>,
    pub tx: Peri<'static, P0_06>,
}

/// Вся плата после init: сгруппированные, уже сконфигурированные пины.
pub struct Board {
    pub leds: LedPins,
    pub buttons: ButtonPins,
    pub temp: Peri<'static, TEMP>,
    pub i2c: I2cPins,
    pub uart: UartPins,
}

/// Разложить «сырые» Peripherals в именованные группы пинов.
/// Конкретные имена пинов (P0_xx / P1_xx) фигурируют ТОЛЬКО здесь.
pub fn split(p: Peripherals) -> Board {
    // ROW init Low: строка по умолчанию неактивна (active-high источник).
    let rows = [
        Output::new(p.P0_21, Level::Low, OutputDrive::Standard), // ROW1
        Output::new(p.P0_22, Level::Low, OutputDrive::Standard), // ROW2
        Output::new(p.P0_15, Level::Low, OutputDrive::Standard), // ROW3
        Output::new(p.P0_24, Level::Low, OutputDrive::Standard), // ROW4
        Output::new(p.P0_19, Level::Low, OutputDrive::Standard), // ROW5
    ];
    // COL init High: столбец по умолчанию неактивен (active-low сток).
    let cols = [
        Output::new(p.P0_28, Level::High, OutputDrive::Standard), // COL1 (P0.28/AIN4, schematic)
        Output::new(p.P0_11, Level::High, OutputDrive::Standard), // COL2 (P0.11, schematic)
        Output::new(p.P0_31, Level::High, OutputDrive::Standard), // COL3 = P0.31/AIN7 (schematic, sheet "Target MCU")
        Output::new(p.P1_05, Level::High, OutputDrive::Standard), // COL4 = P1.05 (порт 1, не P0.05!)
        Output::new(p.P0_30, Level::High, OutputDrive::Standard), // COL5 = P0.30/AIN6 (schematic, sheet "Target MCU")
    ];

    let buttons = ButtonPins {
        a: Input::new(p.P0_14, Pull::Up), // BTN_A
        b: Input::new(p.P0_23, Pull::Up), // BTN_B
    };

    Board {
        leds: LedPins { rows, cols },
        buttons,
        temp: p.TEMP,
        i2c: I2cPins {
            twim: p.TWISPI0,
            scl: p.P0_08, // I2C_INT_SCL (schematic, TP20, R51 1k pull-up)
            sda: p.P0_16, // I2C_INT_SDA (schematic, TP21, R52 1k pull-up)
        },
        uart: UartPins {
            uarte: p.UARTE0,
            tx: p.P0_06, // наш TX = nRF TXD = P0.06 (nrf-rs BSP; = interface RX/UART_INT_RX)
        },
    }
}
