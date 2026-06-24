use embassy_nrf::gpio::{Input, Level, Output, OutputDrive, Pull};
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

/// Вся плата после init: сгруппированные, уже сконфигурированные пины.
pub struct Board {
    pub leds: LedPins,
    pub buttons: ButtonPins,
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
        Output::new(p.P0_28, Level::High, OutputDrive::Standard), // COL1
        Output::new(p.P0_11, Level::High, OutputDrive::Standard), // COL2
        Output::new(p.P0_31, Level::High, OutputDrive::Standard), // COL3
        Output::new(p.P1_05, Level::High, OutputDrive::Standard), // COL4 (порт 1: P1.05, не P0.05!)
        Output::new(p.P0_30, Level::High, OutputDrive::Standard), // COL5
    ];

    let buttons = ButtonPins {
        a: Input::new(p.P0_14, Pull::Up), // BTN_A
        b: Input::new(p.P0_23, Pull::Up), // BTN_B
    };

    Board {
        leds: LedPins { rows, cols },
        buttons,
    }
}
